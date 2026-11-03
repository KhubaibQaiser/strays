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

/// What the loop reaches outside the app state: the Keychain, GitHub and the Mac. Tests swap them.
pub struct Deps {
    pub token: Box<dyn Fn() -> Result<Option<String>, String> + Send>,
    /// GitHub's API, or the fake server in dev runs and tests.
    pub api: String,
    pub full_screen: Box<dyn Fn() -> bool + Send>,
    pub low_power: Box<dyn Fn() -> bool + Send>,
}

impl Deps {
    pub fn real(full_screen: impl Fn() -> bool + Send + 'static) -> Self {
        #[cfg(debug_assertions)]
        let api = std::env::var("STRAYS_FAKE_GITHUB").unwrap_or_else(|_| github::API.into());
        #[cfg(not(debug_assertions))]
        let api = github::API.to_string();
        Self { token: Box::new(crate::token::load), api, full_screen: Box::new(full_screen), low_power: Box::new(crate::platform::low_power_mode) }
    }
}

const UPDATE_EVERY: i64 = 7; // days

/// `on_change` runs after every loop pass that changed something, and after every sync.
const RESCAN_EVERY: i64 = 6; // hours; FSEvents covers the time in between (§5.9)

/// Watches the given folders (not recursively); any change asks the loop to rescan. Also returns what
/// couldn't be watched: those folders are still rescanned every few hours.
fn watch(folders: &[String], tx: &Sender<Cmd>) -> (Option<notify::RecommendedWatcher>, Vec<String>) {
    use notify::Watcher;
    if folders.is_empty() {
        return (None, Vec::new());
    }
    let tx = tx.clone();
    let w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        // the loop has exited when this fails: the app is quitting
        Ok(_) => drop(tx.send(Cmd::Folders)),
        Err(e) => crate::log::warn(&format!("folder watch error: {e}")),
    });
    let mut w = match w {
        Ok(w) => w,
        Err(e) => return (None, vec![format!("Strays can't watch folders for new projects ({e}); it checks every {RESCAN_EVERY} hours instead.")]),
    };
    let failed = folders
        .iter()
        .filter_map(|f| w.watch(std::path::Path::new(f), notify::RecursiveMode::NonRecursive).err().map(|e| format!("Strays can't watch {} ({e}); it checks every {RESCAN_EVERY} hours instead.", crate::state::tilde(f))))
        .collect();
    (Some(w), failed)
}

pub fn run(
    shared: Arc<Mutex<App>>,
    tx: Sender<Cmd>,
    rx: Receiver<Cmd>,
    deps: Deps,
    on_change: impl Fn(&App, Option<SyncOutcome>, Option<Nudge>, Vec<(strays_core::uuid::Uuid, crate::state::Cue)>),
) {
    let mut last_pass = Utc::now();
    let mut last_full: Option<Time> = None;
    let mut not_before: Option<Time> = None;
    let mut forced: Option<Option<strays_core::uuid::Uuid>> = {
        let app = shared.lock().unwrap();
        app.settings.sync_on_launch.then_some(None)
    };
    // the Keychain is read here, off the main thread, rather than on every UI refresh
    let token = (deps.token)();
    // show pets straight away; the first sync may take a while on a slow network
    {
        let mut app = shared.lock().unwrap();
        app.has_token = matches!(token, Ok(Some(_)));
        if let Err(e) = token {
            app.warn("keychain", e);
        }
        app.tick(Utc::now());
        app.refresh_visible(Utc::now());
        on_change(&app, None, None, Vec::new());
    }
    let mut changed = true;
    let mut manual = false;
    let mut watched: Vec<String> = Vec::new();
    let mut watcher = None;
    let mut last_scan: Option<Time> = None;
    let mut sent_today = (now_day(), 0usize);
    let mut fingerprints = crate::state::Fingerprints::new();
    // a new clone or `git init` writes .git a moment after the folder appears, so rescan shortly after events
    let mut rescan_at: Option<Time> = None;
    loop {
        let now = Utc::now();
        // A pass far later than the longest wait means the Mac slept.
        let woke = now - last_pass > Duration::minutes(5);
        last_pass = now;
        let mut app = shared.lock().unwrap();
        // Delete everything closed the store: this thread has nothing left to do
        if app.is_closed() {
            return;
        }
        changed |= app.tick(now);
        if app.settings.watched_folders != watched {
            watched = app.settings.watched_folders.clone();
            let failed;
            (watcher, failed) = watch(&watched, &tx);
            app.resolve("watch");
            if !failed.is_empty() {
                app.warn("watch", failed.join(" "));
            }
            last_scan = None;
        }
        let _ = &watcher; // kept alive for its callbacks
        // folder listings and git reads run without the state, so the UI never waits on the disk
        let scan = (last_scan.is_none_or(|t| now - t > Duration::hours(RESCAN_EVERY)) || rescan_at.is_some_and(|t| now >= t)).then(|| app.settings.watched_folders.clone());
        let jobs = app.local_jobs();
        drop(app);
        let listed = scan.as_deref().map(crate::state::list_git_folders);
        let local = crate::state::read_local(jobs, &mut fingerprints);
        app = shared.lock().unwrap();
        if let Some(listed) = listed {
            changed |= app.apply_scan(listed);
            last_scan = Some(now);
            rescan_at = None;
        }
        changed |= app.apply_local(local);
        if woke && app.settings.sync_on_wake && forced.is_none() {
            forced = Some(None);
        }
        let interval = app.settings.sync_interval.0.map(|m| Duration::minutes(m.into()));
        let wait = if app.sync.failures > 0 { interval.map(|i| i.min(github::backoff(app.sync.failures))) } else { interval };
        // without a token only a manual sync reads the Keychain again; setup sends one after saving
        let scheduled = app.has_token
            && !app.sync.disconnected
            && wait.is_some_and(|w| app.sync.last_attempt.is_none_or(|a| now - a >= w))
            && !(app.settings.sync_pause_on_low_power && (deps.low_power)());
        let allowed = not_before.is_none_or(|t| now >= t);
        let mut outcome = None;
        if app.settings.offline {
            if forced.take().is_some() || app.sync_error.is_none() {
                app.sync_error = Some("Offline mode is on".into());
                changed = true;
            }
        } else if (forced.is_some() || scheduled) && allowed {
            let for_pet = forced.take().flatten();
            let token = (deps.token)();
            app.has_token = matches!(token, Ok(Some(_)));
            match token {
                Err(e) => {
                    app.sync_error = Some(e);
                    outcome = Some(SyncOutcome { ok: false, for_pet });
                }
                Ok(None) => {
                    app.sync_error = Some("No GitHub token yet".into());
                    outcome = Some(SyncOutcome { ok: false, for_pet });
                }
                Ok(Some(token)) => {
                    app.resolve("keychain");
                    // a sync the user asked for is a full one, so "Look again" on a confused pet re-checks its repo
                    let full = manual || last_full.is_none_or(|t| now - t > Duration::hours(FULL_EVERY));
                    let snapshot = app.sync.clone();
                    let tracked = github::tracked_ids(&app.pets);
                    drop(app);
                    // network, without holding the state
                    let reason = if for_pet.is_some() || manual { "manual" } else { "scheduled" };
                    let gh = Github::new(&token, &deps.api).map(|g| g.with_reason(reason));
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
                            let farewells: Vec<_> = app.pets.iter().filter(|p| report.graduated.contains(&p.id)).map(|p| (p.id, crate::state::Cue::graduate(p))).collect();
                            app.cues.extend(farewells);
                            let fresh: Vec<_> = report.new_projects.iter().filter(|r| !app.new_projects.iter().any(|n| n.id == r.id)).cloned().collect();
                            app.new_projects.extend(fresh);
                        }
                        Err(e) => {
                            app.sync_error = Some(e.to_string());
                            if let GhError::RateLimited { retry_after, reset_at } = e {
                                not_before = retry_after.map(|d| now + d).max(*reset_at);
                            }
                        }
                    }
                    app.note_missing();
                    app.save_all();
                    outcome = Some(SyncOutcome { ok: result.is_ok(), for_pet });
                    manual = false;
                }
            }
            changed = true;
        }
        if !app.settings.check_updates {
            // turning it off forgets the last check, so turning it on again checks straight away
            if app.sync.update_checked.is_some() || app.sync.update.is_some() {
                app.sync.update_checked = None;
                app.sync.update = None;
                app.save_all();
                changed = true;
            }
        } else if !app.settings.offline && app.sync.update_checked.is_none_or(|t| now - t >= Duration::days(UPDATE_EVERY)) {
            drop(app);
            // update checks need no token
            let gh = Github::anonymous(&deps.api).map(|g| g.with_reason("update check"));
            let found = gh.as_ref().map_err(Clone::clone).and_then(|g| g.latest_release(github::RELEASES_REPO));
            app = shared.lock().unwrap();
            if let Ok(gh) = &gh {
                app.append_netlog(gh.take_log());
            }
            match found {
                Ok(release) => {
                    app.sync.update_checked = Some(now);
                    app.sync.update = release.filter(|r| github::is_newer(&r.tag_name, env!("CARGO_PKG_VERSION")));
                }
                // try again in a day rather than a week
                Err(_) => app.sync.update_checked = Some(now - Duration::days(UPDATE_EVERY - 1)),
            }
            app.save_all();
            changed = true;
        }
        if !app.settings.error_reports {
            // turning reports off forgets the id, so turning them on again starts a new one
            app.reports.clear();
            if app.sync.report_id.take().is_some() {
                app.save_all();
            }
        } else if !app.settings.offline && !app.reports.is_empty() && crate::report::available() {
            let day = now.date_naive();
            if sent_today.0 != day {
                sent_today = (day, 0);
            }
            let room = crate::report::PER_DAY.saturating_sub(sent_today.1);
            let batch: Vec<_> = std::mem::take(&mut app.reports).into_iter().take(room).collect();
            let id = *app.sync.report_id.get_or_insert_with(strays_core::uuid::Uuid::new_v4);
            app.save_all();
            drop(app);
            let os = crate::platform::os_version();
            let mut log = Vec::new();
            let mut failed = None;
            match crate::report::reporter() {
                Ok(rep) => {
                    for r in &batch {
                        let (result, entry) = rep.send(id, env!("CARGO_PKG_VERSION"), &os, r);
                        log.push(entry);
                        sent_today.1 += 1;
                        if let Err(e) = result {
                            failed = Some(e);
                            break;
                        }
                    }
                }
                Err(e) => failed = Some(e),
            }
            app = shared.lock().unwrap();
            app.append_netlog(log);
            // a report that can't be sent is dropped, not retried: it's never worth a loop of failures
            if let Some(e) = failed {
                crate::log::warn(&format!("error reports not sent: {e}"));
            }
        }
        let nudge = app.next_nudge(now, (deps.full_screen)());
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

fn now_day() -> strays_core::chrono::NaiveDate {
    Utc::now().date_naive()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration as D;

    /// tools/fake-github, as in the core's tests.
    struct Fake(Child, String);

    impl Fake {
        fn start() -> Self {
            let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fake-github/server.py");
            let mut child = Command::new("python3").arg(script).stdout(Stdio::piped()).spawn().expect("python3");
            let mut line = String::new();
            BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
            let port = line.trim().strip_prefix("PORT ").expect("PORT line").to_string();
            Fake(child, format!("http://127.0.0.1:{port}"))
        }
        /// Picks the fake's scenario with a bare HTTP request (the app has no HTTP client of its own).
        fn scenario(&self, name: &str) {
            use std::io::{Read, Write};
            let addr = self.1.strip_prefix("http://").unwrap();
            let mut s = std::net::TcpStream::connect(addr).unwrap();
            let body = format!(r#"{{"name":"{name}"}}"#);
            write!(s, "POST /_scenario HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            assert!(out.starts_with("HTTP/1.0 200") || out.starts_with("HTTP/1.1 200"), "scenario {name}: {out}");
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }

    /// What one `on_change` call saw.
    #[derive(Debug)]
    struct Seen {
        ok: Option<bool>,
        error: Option<String>,
        repos: usize,
        issues: Vec<String>,
    }

    fn start(app: App, deps: Deps) -> (Arc<Mutex<App>>, Sender<Cmd>, mpsc::Receiver<Seen>, std::thread::JoinHandle<()>) {
        let shared = Arc::new(Mutex::new(app));
        let (tx, rx) = mpsc::channel();
        let (seen_tx, seen) = mpsc::channel();
        let (s, t) = (shared.clone(), tx.clone());
        let thread = std::thread::spawn(move || {
            run(s, t, rx, deps, move |app, outcome, _, _| {
                let _ = seen_tx.send(Seen { ok: outcome.map(|o| o.ok), error: app.sync_error.clone(), repos: app.repos.len(), issues: app.issues.iter().map(|i| i.key.clone()).collect() });
            })
        });
        (shared, tx, seen, thread)
    }

    /// The next `on_change` that reports a sync outcome.
    fn next_sync(seen: &mpsc::Receiver<Seen>) -> Seen {
        loop {
            let s = seen.recv_timeout(D::from_secs(20)).expect("a sync outcome");
            if s.ok.is_some() {
                return s;
            }
        }
    }

    fn deps(api: &str, token: Result<Option<String>, String>) -> Deps {
        Deps { token: Box::new(move || token.clone()), api: api.into(), full_screen: Box::new(|| false), low_power: Box::new(|| false) }
    }

    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("strays-loop-{}", strays_core::uuid::Uuid::new_v4()))
    }

    #[test]
    fn the_loop_syncs_on_launch_and_on_request_freezes_on_401_and_stops_when_closed() {
        let fake = Fake::start();
        let dir = temp();
        let (shared, tx, seen, thread) = start(App::load(dir.clone()).unwrap(), deps(&fake.1, Ok(Some("github_pat_fake".into()))));

        // pets are drawn before the first sync, then the launch sync lists every repo
        let first = seen.recv_timeout(D::from_secs(5)).unwrap();
        assert_eq!(first.ok, None);
        let s = next_sync(&seen);
        assert_eq!((s.ok, s.error.as_deref()), (Some(true), None));
        assert!(s.repos > 200, "{s:?}");
        {
            let app = shared.lock().unwrap();
            assert!(app.has_token && app.sync.last_successful_check.is_some());
            assert!(app.netlog().iter().any(|e| e.reason == "scheduled" && e.status == 200), "every request is logged");
        }

        // a 401: the error is shown, the account counts as disconnected, moods freeze at the last good check
        fake.scenario("unauthorized");
        let checked = shared.lock().unwrap().sync.last_successful_check;
        tx.send(Cmd::SyncNow(None)).unwrap();
        let s = next_sync(&seen);
        assert_eq!(s.ok, Some(false));
        assert!(s.error.is_some());
        {
            let app = shared.lock().unwrap();
            assert!(app.sync.disconnected);
            assert_eq!(app.sync.last_successful_check, checked);
        }

        // offline: nothing is fetched, and the user is told why
        shared.lock().unwrap().settings.offline = true;
        let before = shared.lock().unwrap().netlog().len();
        tx.send(Cmd::SyncNow(None)).unwrap();
        let s = loop {
            let s = seen.recv_timeout(D::from_secs(5)).unwrap();
            if s.error.as_deref() == Some("Offline mode is on") {
                break s;
            }
        };
        assert_eq!(s.ok, None);
        assert_eq!(shared.lock().unwrap().netlog().len(), before);

        // Delete everything closes the store: the loop ends on its next pass
        shared.lock().unwrap().close();
        tx.send(Cmd::Changed).unwrap();
        for _ in 0..50 {
            if thread.is_finished() {
                break;
            }
            std::thread::sleep(D::from_millis(100));
        }
        assert!(thread.is_finished(), "the loop stopped");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unreadable_keychain_is_reported_not_taken_for_no_token() {
        let dir = temp();
        // nothing listens here: the loop must not get as far as the network
        let (shared, _tx, seen, _thread) = start(App::load(dir.clone()).unwrap(), deps("http://127.0.0.1:9", Err("Keychain locked".into())));
        let s = next_sync(&seen);
        assert_eq!((s.ok, s.error.as_deref()), (Some(false), Some("Keychain locked")));
        assert!(s.issues.contains(&"keychain".to_string()));
        assert!(!shared.lock().unwrap().has_token);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn without_a_token_nothing_is_scheduled_and_the_user_is_told_on_a_manual_sync() {
        let dir = temp();
        let (shared, tx, seen, _thread) = start(App::load(dir.clone()).unwrap(), deps("http://127.0.0.1:9", Ok(None)));
        let s = next_sync(&seen);
        assert_eq!((s.ok, s.error.as_deref()), (Some(false), Some("No GitHub token yet")));
        tx.send(Cmd::SyncNow(None)).unwrap();
        assert_eq!(next_sync(&seen).error.as_deref(), Some("No GitHub token yet"));
        assert!(shared.lock().unwrap().netlog().is_empty(), "no request without a token");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
