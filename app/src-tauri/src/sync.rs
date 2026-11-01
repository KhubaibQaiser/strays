//! Background loop: ends due snoozes and holidays, runs GitHub syncs on schedule (§6.3), and tells the
//! UI when something changed. Waits are recomputed from timestamps every minute or sooner (§11).
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use strays_core::chrono::{Duration, Utc};
use strays_core::clock::Time;
use strays_core::github::{self, GhError, Github, Listing};
use strays_core::nudge::Nudge;

use crate::state::App;

pub enum Cmd {
    /// User asked for a sync (menu or ⌥⇧-click); `Some(pet)` gets feedback on that pet.
    SyncNow(Option<strays_core::uuid::Uuid>),
    /// State changed in the UI; refresh the overlay and menu.
    Changed,
    /// Something changed inside a watched folder.
    Folders,
}

pub struct SyncOutcome {
    pub ok: bool,
    pub for_pet: Option<strays_core::uuid::Uuid>,
}

const MAX_WAIT: StdDuration = StdDuration::from_secs(60);
const FULL_EVERY: i64 = 24; // hours

fn github_client(token: &str) -> Result<Github, GhError> {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("STRAYS_FAKE_GITHUB") {
        return Github::new(token, &base);
    }
    Github::new(token, github::API)
}

fn error_text(e: &GhError) -> String {
    match e {
        GhError::Unauthorized => "Token expired or revoked".into(),
        GhError::RateLimited { .. } => "GitHub rate limit; waiting".into(),
        GhError::Network(_) => "Offline".into(),
        GhError::WrongTokenType => "Only fine-grained tokens are accepted".into(),
        other => format!("{other:?}"),
    }
}

/// `on_change` runs after every loop pass that changed something, and after every sync.
const RESCAN_EVERY: i64 = 6; // hours; FSEvents covers the time in between (§5.9)

/// Watches the given folders (not recursively); any change asks the loop to rescan.
fn watch(folders: &[String], tx: &Sender<Cmd>) -> Option<notify::RecommendedWatcher> {
    use notify::Watcher;
    if folders.is_empty() {
        return None;
    }
    let tx = tx.clone();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            let _ = tx.send(Cmd::Folders);
        }
    })
    .map_err(|e| eprintln!("folder watching unavailable: {e}"))
    .ok()?;
    for f in folders {
        if let Err(e) = w.watch(std::path::Path::new(f), notify::RecursiveMode::NonRecursive) {
            eprintln!("can't watch {f}: {e}");
        }
    }
    Some(w)
}

pub fn run(
    shared: Arc<Mutex<App>>,
    tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
    full_screen: impl Fn() -> bool,
    on_change: impl Fn(&App, Option<SyncOutcome>, Option<Nudge>, Vec<(strays_core::uuid::Uuid, crate::state::Cue)>),
) {
    let mut last_pass = Utc::now();
    let mut last_full: Option<Time> = None;
    let mut not_before: Option<Time> = None;
    let mut forced: Option<Option<strays_core::uuid::Uuid>> = {
        let app = shared.lock().unwrap();
        app.settings.sync_on_launch.then_some(None)
    };
    // show pets straight away; the first sync may take a while on a slow network
    {
        let mut app = shared.lock().unwrap();
        app.tick(Utc::now());
        app.refresh_visible(Utc::now());
        on_change(&app, None, None, Vec::new());
    }
    let mut changed = true;
    let mut manual = false;
    let mut watched: Vec<String> = Vec::new();
    let mut watcher = None;
    let mut last_scan: Option<Time> = None;
    // a new clone or `git init` writes .git a moment after the folder appears, so rescan shortly after events
    let mut rescan_at: Option<Time> = None;
    loop {
        let now = Utc::now();
        // A pass far later than the longest wait means the Mac slept.
        let woke = now - last_pass > Duration::minutes(5);
        last_pass = now;
        let mut app = shared.lock().unwrap();
        changed |= app.tick(now);
        if app.settings.watched_folders != watched {
            watched = app.settings.watched_folders.clone();
            watcher = watch(&watched, &tx);
            last_scan = None;
        }
        let _ = &watcher; // kept alive for its callbacks
        if last_scan.is_none_or(|t| now - t > Duration::hours(RESCAN_EVERY)) || rescan_at.is_some_and(|t| now >= t) {
            changed |= app.scan_folders();
            last_scan = Some(now);
            rescan_at = None;
        }
        changed |= app.refresh_local_activity();
        if woke && app.settings.sync_on_wake && forced.is_none() {
            forced = Some(None);
        }
        let interval = app.settings.sync_interval.0.map(|m| Duration::minutes(m.into()));
        let wait = if app.sync.failures > 0 { interval.map(|i| i.min(github::backoff(app.sync.failures))) } else { interval };
        let scheduled = !app.sync.disconnected
            && wait.is_some_and(|w| app.sync.last_attempt.is_none_or(|a| now - a >= w))
            && !(app.settings.sync_pause_on_low_power && crate::platform::low_power_mode());
        let allowed = not_before.is_none_or(|t| now >= t);
        let mut outcome = None;
        if app.settings.offline {
            if forced.take().is_some() || app.sync_error.is_none() {
                app.sync_error = Some("Offline mode is on".into());
                changed = true;
            }
        } else if (forced.is_some() || scheduled) && allowed {
            let for_pet = forced.take().flatten();
            match crate::token::load() {
                None => {
                    app.sync_error = Some("No GitHub token yet".into());
                    outcome = Some(SyncOutcome { ok: false, for_pet });
                }
                Some(token) => {
                    let full = last_full.is_none_or(|t| now - t > Duration::hours(FULL_EVERY));
                    let snapshot = app.sync.clone();
                    let tracked = github::tracked_ids(&app.pets);
                    drop(app);
                    // network, without holding the state
                    let reason = if for_pet.is_some() || manual { "manual" } else { "scheduled" };
                    let gh = github_client(&token).map(|g| g.with_reason(reason));
                    let result = gh.as_ref().map_err(Clone::clone).and_then(|gh| github::fetch(gh, &snapshot, &tracked, full).map(|f| (f, gh.meta())));
                    app = shared.lock().unwrap();
                    if let Ok(gh) = &gh {
                        app.append_netlog(gh.take_log());
                    }
                    let (fetched, meta) = match result {
                        Ok((f, m)) => (Ok(f), m),
                        Err(e) => (Err(e), Default::default()),
                    };
                    if let Ok(f) = &fetched
                        && let Listing::Changed { repos, .. } = &f.listing
                    {
                        merge_repos(&mut app, repos);
                    }
                    let mut pets = std::mem::take(&mut app.pets);
                    let settings = app.settings.clone();
                    let result = github::apply(fetched, &meta, &mut app.sync, &mut pets, &settings, Utc::now());
                    app.pets = pets;
                    match &result {
                        Ok(report) => {
                            if full {
                                last_full = Some(now);
                            }
                            app.sync_error = None;
                            not_before = None;
                            let fed: Vec<_> = report.updated.iter().map(|id| (*id, crate::state::Cue::Eat)).collect();
                            app.cues.extend(fed);
                            let fresh: Vec<_> = report.new_projects.iter().filter(|r| !app.new_projects.iter().any(|n| n.id == r.id)).cloned().collect();
                            app.new_projects.extend(fresh);
                        }
                        Err(e) => {
                            app.sync_error = Some(error_text(e));
                            if let GhError::RateLimited { retry_after, reset_at } = e {
                                not_before = retry_after.map(|d| now + d).max(*reset_at);
                            }
                        }
                    }
                    app.save_all();
                    outcome = Some(SyncOutcome { ok: result.is_ok(), for_pet });
                    manual = false;
                }
            }
            changed = true;
        }
        let nudge = app.next_nudge(now, full_screen());
        if changed || nudge.is_some() {
            let was_visible = app.visible.clone();
            app.refresh_visible(now);
            let cues = app.take_cues(&was_visible);
            on_change(&app, outcome, nudge, cues);
            changed = false;
        }
        let until_deadline = app.next_deadline().map(|d| (d - now).to_std().unwrap_or_default());
        drop(app);
        let until_rescan = rescan_at.map(|t| (t - now).to_std().unwrap_or_default());
        let timeout = [until_deadline, until_rescan].into_iter().flatten().fold(MAX_WAIT, StdDuration::min).max(StdDuration::from_millis(200));
        match rx.recv_timeout(timeout) {
            Ok(Cmd::SyncNow(pet)) => {
                forced = Some(pet);
                manual = true;
            }
            Ok(Cmd::Changed) => changed = true,
            Ok(Cmd::Folders) => rescan_at = Some(Utc::now() + Duration::seconds(2)),
            // moods drift with time even when nothing happens
            Err(RecvTimeoutError::Timeout) => changed = true,
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Keeps `app.repos` as the latest view of every repo, newest push first.
fn merge_repos(app: &mut App, repos: &[github::Repo]) {
    for r in repos {
        match app.repos.iter_mut().find(|x| x.id == r.id) {
            Some(x) => *x = r.clone(),
            None => app.repos.push(r.clone()),
        }
    }
    app.repos.sort_by_key(|r| std::cmp::Reverse(r.pushed_at));
}
