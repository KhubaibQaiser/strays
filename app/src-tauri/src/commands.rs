//! Commands the windows call. The token goes in through `token_check` / `token_save` and never comes back out.
use serde::Serialize;
use strays_core::chrono::{Local, Utc};
use strays_core::clock::Time;
use strays_core::github::{self, Github, NetEntry};
use strays_core::mood::Pace;
use strays_core::pet::{Goal, Pet, Species, StateKind};
use strays_core::settings::{Liveliness, Settings};
use strays_core::uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::Shared;
use crate::state::{Account, AdoptRequest, App, mood_word};
use crate::sync::Cmd;
use crate::{menu, windows};

type Res<T> = Result<T, String>;

/// A command's failure is shown by the window that asked; it goes to the log too.
fn logged<T>(command: &str, r: Res<T>) -> Res<T> {
    r.inspect_err(|e| crate::log::warn(&format!("{command}: {e}")))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPet {
    id: Uuid,
    name: String,
    species: Species,
    colors: std::collections::BTreeMap<String, String>,
    state: StateKind,
    until: Option<Time>,
    since: Time,
    mood: Option<String>,
    idle_days: f64,
    /// Days until the mood drops a level; `None` when already Gloomy or not Active.
    next_mood_days: Option<f64>,
    repo: Option<String>,
    folder: Option<String>,
    sources: Vec<&'static str>,
    last_activity: Time,
    last_source: String,
    goal: Goal,
    pace: Pace,
    liveliness: Option<Liveliness>,
    on_screen: bool,
    releases: usize,
    /// GitHub can't find the repo: deleted, or the token can't see it.
    missing: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiRepo {
    id: u64,
    full_name: String,
    pushed_at: Option<Time>,
    private: bool,
    tracked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pets: Vec<UiPet>,
    settings: Settings,
    account: Option<Account>,
    has_token: bool,
    last_sync: Option<Time>,
    sync_error: Option<String>,
    rate_remaining: Option<u32>,
    requests_today: usize,
    repos: Vec<UiRepo>,
    new_projects: Vec<UiRepo>,
    /// Git folders inside the watched folders: (path, shown as, has a pet).
    local_repos: Vec<(String, String, bool)>,
    new_folders: Vec<String>,
    slots_used: usize,
    presentation: bool,
    vacation: bool,
    hide_all: bool,
    data_dir: String,
    version: &'static str,
    login_item: crate::platform::LoginItem,
    /// A newer release found by the opt-in update check, and when it last ran.
    update: Option<strays_core::github::Release>,
    update_checked: Option<Time>,
    /// Problems to show until fixed or dismissed.
    issues: Vec<String>,
    /// Whether this build can send error reports (it has a PostHog key).
    reports_available: bool,
    pub first_run: bool,
}

fn ui_pet(app: &App, p: &Pet, now: Time) -> UiPet {
    let active = p.state.kind == StateKind::Active;
    let idle = p.idle_days(now, app.sync.last_successful_check, &app.settings, &Local);
    let mood = app.mood(p, now);
    let th = p.thresholds(&app.settings).0;
    let next = (active && (mood as usize) < 5).then(|| (th[mood as usize - 1] - idle).max(0.0));
    let mut sources = Vec::new();
    if p.project.github.is_some() {
        sources.push("GitHub");
    }
    if p.project.local.is_some() {
        sources.push(if p.project.github.is_some() { "Local" } else { "This Mac only" });
    }
    UiPet {
        id: p.id,
        name: p.name.clone(),
        species: p.species,
        colors: p.colors.clone(),
        state: p.state.kind,
        until: p.state.until,
        since: p.state.since,
        mood: active.then(|| mood_word(mood).to_string()),
        idle_days: idle,
        next_mood_days: next,
        repo: p.project.github.as_ref().map(|g| g.full_name.clone()),
        folder: p.project.local.as_ref().map(|l| l.display_path.clone()),
        sources,
        last_activity: p.activity.last,
        last_source: format!("{:?}", p.activity.source),
        goal: p.goal.clone(),
        pace: p.pace,
        liveliness: p.liveliness,
        on_screen: app.visible.contains(&p.id),
        releases: p.releases.len(),
        missing: p.missing_since.is_some(),
    }
}

fn repo_view(app: &App, r: &github::Repo) -> UiRepo {
    UiRepo { id: r.id, full_name: r.full_name.clone(), pushed_at: r.pushed_at, private: r.private, tracked: app.tracked(r.id) }
}

pub fn snapshot(app: &App) -> UiState {
    let now = Utc::now();
    let today = now.with_timezone(&Local).date_naive();
    UiState {
        pets: app.pets.iter().map(|p| ui_pet(app, p, now)).collect(),
        settings: app.settings.clone(),
        account: app.account.clone(),
        has_token: app.has_token,
        last_sync: app.sync.last_successful_check,
        sync_error: app.sync_error.clone(),
        rate_remaining: app.sync.rate_remaining,
        requests_today: app.netlog().iter().filter(|e| e.at.with_timezone(&Local).date_naive() == today).count(),
        repos: app.repos.iter().filter(|r| !r.archived).map(|r| repo_view(app, r)).collect(),
        new_projects: app.new_projects.iter().map(|r| repo_view(app, r)).collect(),
        local_repos: app.local_repos.iter().map(|p| (p.clone(), crate::state::tilde(p), app.tracked_folder(p))).collect(),
        new_folders: app.new_folders.clone(),
        slots_used: strays_core::pet::slots_used(&app.pets),
        presentation: app.flags.presentation,
        vacation: app.flags.vacation,
        hide_all: app.flags.hide_all,
        data_dir: app.dir.display().to_string(),
        version: env!("CARGO_PKG_VERSION"),
        login_item: crate::platform::login_item(),
        update: app.sync.update.clone(),
        update_checked: app.sync.update_checked,
        issues: app.issues.iter().map(|i| i.text.clone()).collect(),
        reports_available: crate::report::available(),
        first_run: app.pets.is_empty() && app.account.is_none() && app.settings.watched_folders.is_empty(),
    }
}

/// Tells every window and the overlay that state changed.
fn changed(shared: &Shared) {
    let _ = shared.tx.send(Cmd::Changed);
}

#[tauri::command]
pub fn ui_state(shared: State<'_, Shared>) -> UiState {
    snapshot(&shared.app.lock().unwrap())
}

/// Same action ids as the menus: `snooze-1h`, `wake`, `release`, …
#[tauri::command]
pub fn pet_action(id: Uuid, action: String, shared: State<'_, Shared>) -> Res<String> {
    logged("pet_action", (move || -> Res<String> {
        let event = menu::parse(&format!("pet/{id}/{action}")).ok_or("Unknown action")?;
        match event {
            menu::Event::SyncPet(pet) => {
                let _ = shared.tx.send(Cmd::SyncNow(Some(pet)));
                Ok("Syncing…".into())
            }
            menu::Event::Pet(pet, action) => {
                let r = shared.app.lock().unwrap().act(pet, action, Utc::now());
                changed(&shared);
                r
            }
            _ => Err("Unknown action".into()),
        }
    })())
}

#[tauri::command]
pub fn set_goal(id: Uuid, goal: Goal, shared: State<'_, Shared>) -> Res<()> {
    logged("set_goal", (move || -> Res<()> {
        shared.app.lock().unwrap().set_goal(id, goal)?;
        changed(&shared);
        Ok(())
    })())
}

#[tauri::command]
pub fn set_pet(id: Uuid, name: String, pace: Pace, liveliness: Option<Liveliness>, shared: State<'_, Shared>) -> Res<()> {
    logged("set_pet", (move || -> Res<()> {
        shared.app.lock().unwrap().set_pet(id, name, pace, liveliness)?;
        changed(&shared);
        Ok(())
    })())
}

#[tauri::command]
pub fn adopt(request: AdoptRequest, shared: State<'_, Shared>) -> Res<String> {
    logged("adopt", (move || -> Res<String> {
        let r = shared.app.lock().unwrap().adopt(request, Utc::now());
        changed(&shared);
        r
    })())
}

#[tauri::command]
pub fn save_settings(settings: Settings, shared: State<'_, Shared>) -> Res<()> {
    logged("save_settings", (move || -> Res<()> {
        let r = shared.app.lock().unwrap().save_settings(settings);
        changed(&shared);
        r
    })())
}

#[tauri::command]
pub fn set_flag(name: String, on: bool, shared: State<'_, Shared>) -> Res<()> {
    logged("set_flag", (move || -> Res<()> {
        let mut app = shared.app.lock().unwrap();
        match name.as_str() {
            "presentation" => app.flags.presentation = on,
            "hideAll" => app.flags.hide_all = on,
            "vacation" => app.set_vacation(on, Utc::now()),
            _ => return Err("Unknown setting".into()),
        }
        drop(app);
        changed(&shared);
        Ok(())
    })())
}

/// Settings → Sound → Test: plays a greeting at the chosen volume, even with sound off.
#[tauri::command]
pub fn test_sound(shared: State<'_, Shared>) -> Res<()> {
    logged("test_sound", (move || -> Res<()> {
        let volume = shared.app.lock().map_err(|_| "busy")?.settings.volume;
        crate::audio::check_output()?;
        shared.audio.play(crate::audio::Sound::Greeting(strays_core::pet::Species::Cat), volume);
        Ok(())
    })())
}

/// Settings → Privacy → Launch at login. macOS keeps the state (System Settings → Login Items), so nothing is saved here.
#[tauri::command]
pub fn set_login_item(on: bool, h: AppHandle) -> Res<crate::platform::LoginItem> {
    logged("set_login_item", (move || -> Res<crate::platform::LoginItem> {
        let state = crate::platform::set_login_item(on)?;
        emit_changed(&h);
        Ok(state)
    })())
}

/// Settings → "Dismiss": hides the current problems; any that happen again come back.
#[tauri::command]
pub fn dismiss_issues(shared: State<'_, Shared>) {
    shared.app.lock().unwrap().issues.clear();
    changed(&shared);
}

#[tauri::command]
pub fn sync_now(shared: State<'_, Shared>) {
    let _ = shared.tx.send(Cmd::SyncNow(None));
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCheck {
    login: String,
    repo_count: u64,
    expires: Option<Time>,
}

fn client(token: &str) -> Res<Github> {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("STRAYS_FAKE_GITHUB") {
        return Github::new(token, &base).map(|g| g.with_reason("validation")).map_err(|e| e.to_string());
    }
    Github::new(token.trim(), github::API).map(|g| g.with_reason("validation")).map_err(|e| match e {
        github::GhError::WrongTokenType => "That isn't a fine-grained token. It should start with github_pat_".into(),
        other => other.to_string(),
    })
}

fn describe(e: github::GhError) -> String {
    match e {
        github::GhError::Unauthorized => "GitHub didn't accept this token. Check it was copied in full and hasn't expired.".into(),
        github::GhError::Network(_) => "Couldn't reach GitHub. Check your connection.".into(),
        github::GhError::RateLimited { .. } => "GitHub's rate limit was hit. Try again in a few minutes.".into(),
        other => other.to_string(),
    }
}

/// Setup step 2 (§6.2): checks a pasted token without saving it.
#[tauri::command]
pub async fn token_check(token: String, shared: State<'_, Shared>) -> Res<TokenCheck> {
    logged("token_check", async move {
        let app = shared.app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let gh = client(&token)?;
            let v = gh.validate();
            app.lock().unwrap().append_netlog(gh.take_log());
            let v = v.map_err(describe)?;
            Ok(TokenCheck { login: v.login, repo_count: v.repo_count, expires: v.expires })
        })
        .await
        .map_err(|e| e.to_string())?
    }.await)
}

/// Setup step 4: validates again, stores the token in the Keychain and starts the first sync.
#[tauri::command]
pub async fn token_save(token: String, shared: State<'_, Shared>) -> Res<()> {
    logged("token_save", async move {
        let app = shared.app.clone();
        let tx = shared.tx.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let gh = client(&token)?;
            let v = gh.validate();
            let mut app = app.lock().unwrap();
            if app.is_closed() {
                return Err("Strays is deleting its data".into());
            }
            app.append_netlog(gh.take_log());
            let v = v.map_err(describe)?;
            crate::token::save(token.trim())?;
            app.has_token = true;
            app.set_account(Some(Account { login: v.login, expires: v.expires, repo_count: v.repo_count }));
            app.sync.disconnected = false;
            app.sync.failures = 0;
            app.sync_error = None;
            drop(app);
            let _ = tx.send(Cmd::SyncNow(None));
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?
    }.await)
}

/// Deletes the Keychain item straight away (§6.4). Pets keep their data and freeze.
#[tauri::command]
pub fn token_disconnect(shared: State<'_, Shared>) -> Res<()> {
    logged("token_disconnect", (move || -> Res<()> {
        crate::token::delete()?;
        let mut app = shared.app.lock().unwrap();
        app.has_token = false;
        app.set_account(None);
        // what the old token could see isn't offered any more
        app.merge_repos(&[], true);
        app.sync_error = Some("Not connected to GitHub".into());
        drop(app);
        changed(&shared);
        Ok(())
    })())
}

#[tauri::command]
pub async fn pick_folder(h: AppHandle) -> Option<String> {
    let path = h.dialog().file().set_title("Choose a project folder").blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())?;
    // picking is the only moment the sandbox grants access; the bookmark keeps it across restarts
    #[cfg(target_os = "macos")]
    {
        let shared = h.state::<Shared>();
        let mut app = shared.app.lock().unwrap();
        app.allow_folder(path.clone());
        match crate::platform::bookmark(&path) {
            Ok(data) => {
                app.resolve(&format!("access:{path}"));
                app.remember_bookmark(path.clone(), data);
            }
            Err(e) => app.warn(&format!("access:{path}"), format!("Strays can read {} now, but macOS refused to remember it ({e}), so it'll need adding again after a restart.", crate::state::tilde(&path))),
        }
        drop(app);
        changed(&shared);
    }
    Some(path)
}

#[tauri::command]
pub fn open_github(id: Uuid, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    logged("open_github", (move || -> Res<()> {
        let app = shared.app.lock().unwrap();
        let repo = app.pets.iter().find(|p| p.id == id).and_then(|p| p.project.github.clone()).ok_or("Not on GitHub")?;
        h.opener().open_url(format!("https://github.com/{}", repo.full_name), None::<&str>).map_err(|e| e.to_string())
    })())
}

#[tauri::command]
pub fn reveal(id: Uuid, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    logged("reveal", (move || -> Res<()> {
        let app = shared.app.lock().unwrap();
        let path = app.pets.iter().find(|p| p.id == id).and_then(|p| p.project.local.clone()).map(|l| l.path).ok_or("No local folder")?;
        h.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
    })())
}

/// Opens one of a fixed set of pages in the browser; the web view can't name arbitrary URLs.
#[tauri::command]
pub fn open_link(which: String, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    logged("open_link", (move || -> Res<()> {
        let url = match which.as_str() {
            "token-page" => "https://github.com/settings/personal-access-tokens/new".to_string(),
            "new-issue" => format!("https://github.com/{}/issues/new", strays_core::github::RELEASES_REPO),
            // the page comes from GitHub's answer, so only this repo's release pages are opened
            "release" => shared.app.lock().unwrap().sync.update.as_ref().map(|r| r.html_url.clone())
                .filter(|u| u.starts_with(&format!("https://github.com/{}/releases/", strays_core::github::RELEASES_REPO)))
                .ok_or("No update to download")?,
            _ => return Err("Unknown link".into()),
        };
        h.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    })())
}

#[tauri::command]
pub fn netlog(shared: State<'_, Shared>) -> Vec<NetEntry> {
    let mut log = shared.app.lock().unwrap().netlog();
    log.reverse();
    log
}

/// Lines of the log a "Send log" carries: enough for the last few days, small enough to read first.
const SENT_LINES: usize = 500;
/// The log the user last previewed: "Send" sends exactly this, not a newer one.
static PREVIEWED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The log made safe to share: a launch-style header, both log files, the last crash and recent network
/// activity, with every path, pet, repo, account and folder name blanked. `tail` keeps only the last lines.
pub(crate) fn shareable_log(app: &App, tail: Option<usize>) -> String {
    let private = app.private_words();
    let private: Vec<&str> = private.iter().map(String::as_str).collect();
    let mut out = vec![format!("Strays {} on macOS {}, {} · saved {}", env!("CARGO_PKG_VERSION"), crate::platform::os_version(), std::env::consts::ARCH, Utc::now().to_rfc3339())];
    let log = crate::log::path();
    let files = [log.as_ref().map(|p| p.with_extension("log.1")), log, Some(app.dir.join(crate::report::CRASH_FILE))];
    for path in files.into_iter().flatten() {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                out.push(format!("--- {name}"));
                out.extend(text.lines().map(String::from));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => out.push(format!("--- {name} couldn't be read: {e}")),
        }
    }
    let net = app.netlog();
    out.push("--- network activity (latest 50)".into());
    out.extend(net.iter().rev().take(50).rev().map(|e| format!("{} {} {}{} {} {}", e.at.to_rfc3339(), e.method, e.host, e.path, e.status, e.reason)));
    let start = tail.map_or(0, |n| out.len().saturating_sub(n));
    let mut lines: Vec<String> = out[start..].iter().map(|l| strays_core::report::scrub(l, &private)).collect();
    if start > 0 {
        lines.insert(0, out[0].clone());
    }
    lines.join("\n") + "\n"
}

/// Settings → Privacy → "Save log for a bug report": the scrubbed log as one file, revealed in Finder.
#[tauri::command]
pub fn save_log(shared: State<'_, Shared>, h: AppHandle) -> Res<String> {
    logged("save_log", (move || -> Res<String> {
        let app = shared.app.lock().unwrap();
        let path = app.dir.join(format!("strays-log-{}.txt", Utc::now().format("%Y%m%d-%H%M%S")));
        std::fs::write(&path, shareable_log(&app, None)).map_err(|e| format!("Couldn't save the log: {e}"))?;
        drop(app);
        h.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string())?;
        Ok(path.display().to_string())
    })())
}

/// What "Send log" would send, for the user to read first.
#[tauri::command]
pub fn log_preview(shared: State<'_, Shared>) -> Res<String> {
    logged("log_preview", (move || -> Res<String> {
        if !crate::report::available() {
            return Err("This build can't send logs; save it for a bug report instead".into());
        }
        let text = shareable_log(&shared.app.lock().unwrap(), Some(SENT_LINES));
        *PREVIEWED.lock().unwrap() = Some(text.clone());
        Ok(text)
    })())
}

/// Sends the log the user just previewed to the developer (PostHog), once, when they ask.
#[tauri::command]
pub async fn send_log(shared: State<'_, Shared>) -> Res<()> {
    logged("send_log", async move {
        let text = PREVIEWED.lock().unwrap().take().ok_or("Look at the log first (Send log…)")?;
        let id = {
            let mut app = shared.app.lock().unwrap();
            if app.settings.offline {
                return Err("Offline mode is on".into());
            }
            let id = *app.sync.report_id.get_or_insert_with(strays_core::uuid::Uuid::new_v4);
            app.save_all();
            id
        };
        let os = crate::platform::os_version();
        let (result, entry) = tauri::async_runtime::spawn_blocking(move || match crate::report::reporter() {
            Ok(rep) => {
                let (r, e) = rep.send_log(id, env!("CARGO_PKG_VERSION"), &os, &text);
                (r, Some(e))
            }
            Err(e) => (Err(e), None),
        })
        .await
        .map_err(|e| e.to_string())?;
        let mut app = shared.app.lock().unwrap();
        app.append_netlog(entry.into_iter().collect());
        drop(app);
        let _ = shared.tx.send(crate::Cmd::Changed);
        result.map_err(|e| format!("The log wasn't sent: {e}"))
    }.await)
}

/// Settings → Privacy → "Show log": the file to attach to a bug report.
#[tauri::command]
pub fn reveal_log(h: AppHandle) -> Res<()> {
    logged("reveal_log", (move || -> Res<()> {
        let path = crate::log::path().filter(|p| p.exists()).ok_or("Nothing has been logged yet")?;
        h.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
    })())
}

#[tauri::command]
pub fn open_data_folder(shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    logged("open_data_folder", (move || -> Res<()> {
        let dir = shared.app.lock().unwrap().dir.clone();
        h.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
    })())
}

/// Writes every pet, the settings and the sync state into one JSON file and reveals it. No token.
#[tauri::command]
pub fn export_data(shared: State<'_, Shared>, h: AppHandle) -> Res<String> {
    logged("export_data", (move || -> Res<String> {
        let app = shared.app.lock().unwrap();
        let export = serde_json::json!({ "pets": app.pets, "settings": app.settings, "sync": app.sync, "account": app.account });
        let path = app.dir.join(format!("strays-export-{}.json", Utc::now().format("%Y%m%d-%H%M%S")));
        std::fs::write(&path, serde_json::to_vec_pretty(&export).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        h.opener().reveal_item_in_dir(&path).map_err(|e| format!("Exported to {}, but Finder couldn't show it: {e}", path.display()))?;
        Ok(path.display().to_string())
    })())
}

/// §8.5 "Delete everything": the Keychain item, every data file and log, then quit.
#[tauri::command]
pub fn delete_everything(confirm: String, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    logged("delete_everything", (move || -> Res<()> {
        if confirm != "DELETE" {
            return Err("Type DELETE to confirm".into());
        }
        // held until the end: every write to the data folder takes this lock, and the closed store refuses
        // them afterwards, so no save (sync thread, a window, a late token save) can recreate what's deleted
        let mut app = shared.app.lock().unwrap();
        app.close();
        crate::log::stop();
        let mut failed = Vec::new();
        if let Err(e) = crate::token::delete() {
            failed.push(e);
        }
        if crate::platform::login_item() != crate::platform::LoginItem::Off
            && let Err(e) = crate::platform::set_login_item(false)
        {
            failed.push(format!("the login item ({e})"));
        }
        let mut dirs = vec![app.dir.clone()];
        // inside the sandbox, home is the app's own container, so its web view data is Strays' alone
        if std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some()
            && let Some(home) = std::env::var_os("HOME")
        {
            let lib = std::path::Path::new(&home).join("Library");
            dirs.extend(["WebKit", "Caches/WebKit"].map(|d| lib.join(d)));
        }
        for d in dirs {
            match std::fs::remove_dir_all(&d) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => failed.push(format!("{} ({e})", d.display())),
                _ => {}
            }
        }
        // quitting would hide the failure; staying open lets the user see it and try again
        drop(app);
        // the sync thread sees the closed store on its next pass and ends
        let _ = shared.tx.send(Cmd::Changed);
        if !failed.is_empty() {
            return Err(format!("Some things couldn't be deleted: {}. Try again, or remove them by hand and quit Strays: it no longer saves anything.", failed.join("; ")));
        }
        h.exit(0);
        Ok(())
    })())
}

#[tauri::command]
pub fn open_window(kind: String, arg: Option<String>, h: AppHandle) -> Res<()> {
    logged("open_window", (move || -> Res<()> {
        let arg = arg.unwrap_or_default();
        match kind.as_str() {
            "settings" => windows::settings(&h, if arg.is_empty() { "pets" } else { &arg }),
            "setup" => windows::setup(&h),
            "adopt" => windows::adopt(&h, &arg),
            "welcome" => windows::welcome(&h),
            _ => return Err("Unknown window".into()),
        }
        .map_err(|e| e.to_string())
    })())
}

#[tauri::command]
pub fn close_window(label: String, window: tauri::Window, h: AppHandle) -> Res<()> {
    logged("close_window", (move || -> Res<()> {
        // a page closes its own window, never another (the overlay, Settings)
        if label != window.label() {
            return Err(format!("The {} window can't close the {label} window", window.label()));
        }
        match h.get_webview_window(&label) {
            Some(w) => w.close().map_err(|e| e.to_string()),
            None => Ok(()),
        }
    })())
}

/// Windows re-fetch `ui_state` when they hear this.
pub fn emit_changed(h: &AppHandle) {
    if let Err(e) = h.emit("state-changed", ()) {
        crate::log::warn(&format!("could not tell the windows about a change: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, Value, json};
    use strays_core::github::{Owner, Release, Repo};
    use strays_core::pet::{Activity, ActivitySource, GithubRef, LocalRef, Project};

    /// The JSON shape of a value: field names and the kinds of value each holds ("string", "null|number", …),
    /// arrays as the merged shape of their items. Empty objects stand for maps with any keys.
    fn shape(v: &Value) -> Value {
        match v {
            Value::Null => json!("null"),
            Value::Bool(_) => json!("boolean"),
            Value::Number(_) => json!("number"),
            Value::String(_) => json!("string"),
            Value::Array(items) => json!([items.iter().map(shape).reduce(|a, b| merge(&a, &b)).unwrap_or(json!("never"))]),
            Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), shape(v))).collect()),
        }
    }

    fn merge(a: &Value, b: &Value) -> Value {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let mut out = Map::new();
                for k in x.keys().chain(y.keys()) {
                    let v = match (x.get(k), y.get(k)) {
                        (Some(p), Some(q)) => merge(p, q),
                        (Some(p), None) | (None, Some(p)) => p.clone(),
                        (None, None) => unreachable!(),
                    };
                    out.insert(k.clone(), v);
                }
                Value::Object(out)
            }
            // an empty array's items: no evidence either way
            (Value::String(n), other) | (other, Value::String(n)) if n == "never" => other.clone(),
            (Value::Array(x), Value::Array(y)) => json!([merge(&x[0], &y[0])]),
            (Value::String(x), Value::String(y)) => {
                let mut kinds: Vec<&str> = x.split('|').chain(y.split('|')).collect();
                kinds.sort();
                kinds.dedup();
                json!(kinds.join("|"))
            }
            // an object or array on one side and null on the other: an optional structure
            (Value::String(n), other) | (other, Value::String(n)) if n == "null" => json!({ "nullable": other }),
            (Value::Object(o), other) | (other, Value::Object(o)) if o.contains_key("nullable") => json!({ "nullable": merge(&o["nullable"], other) }),
            (x, y) => panic!("can't merge {x} and {y}"),
        }
    }

    /// Every optional filled in: pets in two states, an account, repos, folders, an update, a problem.
    #[test]
    fn a_shared_log_carries_no_names_paths_or_folders() {
        let dir = std::env::temp_dir().join(format!("strays-sharelog-{}", Uuid::new_v4()));
        let mut app = full_app(&dir);
        app.settings.watched_folders = vec!["/Users/me/Code/ClientX-NDA".into()];
        std::fs::write(dir.join(crate::report::CRASH_FILE), "src/state.rs:1\nThe file “ClientX-NDA” couldn't be opened for Miso and Rex in me/strays\n").unwrap();
        let text = shareable_log(&app, None);
        assert!(text.starts_with(&format!("Strays {}", env!("CARGO_PKG_VERSION"))), "{text}");
        assert!(text.contains("couldn't be opened"), "the message itself stays: {text}");
        for private in ["ClientX-NDA", "Miso", "Rex", "me/strays", "/Users/me"] {
            assert!(!text.contains(private), "{private} leaked: {text}");
        }
        // a preview keeps the header and only the newest lines
        assert_eq!(shareable_log(&app, Some(1)).lines().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn full_app(dir: &std::path::Path) -> App {
        let mut app = App::load(dir.to_path_buf()).unwrap();
        let now = Utc::now();
        let gh = GithubRef { id: 7, full_name: "me/strays".into(), account: "me".into() };
        let local = LocalRef { bookmark_id: Uuid::new_v4(), display_path: "~/Code/x".into(), path: "/Users/me/Code/x".into() };
        let mut a = Pet::new(now, "Miso", Species::Cat, Project { github: Some(gh), local: Some(local) }, Activity { last: now, source: ActivitySource::Github }, Pace::Normal);
        a.adopt(now).unwrap();
        a.missing_since = Some(now);
        let mut b = Pet::new(now, "Rex", Species::Dog, Project::default(), Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Sprint);
        b.adopt(now).unwrap();
        b.snooze(now, Some(now + strays_core::chrono::Duration::hours(1))).unwrap();
        app.pets = vec![a, b];
        app.account = Some(Account { login: "me".into(), expires: Some(now), repo_count: 3 });
        app.sync.last_successful_check = Some(now);
        app.sync.rate_remaining = Some(4000);
        app.sync.update = Some(Release { tag_name: "v0.2.0".into(), html_url: "https://github.com/KhubaibQaiser/strays/releases/tag/v0.2.0".into() });
        app.sync.update_checked = Some(now);
        app.sync_error = Some("offline".into());
        let repo = Repo { id: 9, name: "x".into(), full_name: "me/x".into(), owner: Owner { login: "me".into() }, private: true, fork: false, archived: false, created_at: now, pushed_at: Some(now) };
        app.repos = vec![repo.clone()];
        app.new_projects = vec![repo];
        app.local_repos = vec!["/Users/me/Code/x".into()];
        app.new_folders = vec!["/Users/me/Code/y".into()];
        app.warn("x", "a problem");
        app.refresh_visible(now);
        app
    }

    /// The contract the TypeScript side is checked against (tests/contract.spec.ts): the shape of every
    /// payload the windows and the overlay receive. `UPDATE_CONTRACT=1 cargo test contract` rewrites it.
    #[test]
    fn contract() {
        let dir = std::env::temp_dir().join(format!("strays-contract-{}", Uuid::new_v4()));
        let full = full_app(&dir.join("full"));
        let empty = App::load(dir.join("empty")).unwrap();
        let ui = merge(&shape(&serde_json::to_value(snapshot(&full)).unwrap()), &shape(&serde_json::to_value(snapshot(&empty)).unwrap()));
        let placed = |confused| crate::overlay::Placed { view: crate::state::PetView { id: Uuid::new_v4(), species: Species::Cat, colors: Default::default(), label: "Miso".into(), mood: strays_core::mood::Mood::Content, confused, liveliness: None, home: None }, x: 1.0, y: 2.0, s: 3.0 };
        let frame = crate::overlay::Frame { pets: vec![placed(true)], grid: Default::default(), liveliness: strays_core::settings::Liveliness::Calm, reduce_motion: false, hidden: false };
        let hovers = [crate::overlay::Hover { id: Some(Uuid::new_v4()), held: true }, crate::overlay::Hover { id: None, held: false }];
        let hover = merge(&shape(&serde_json::to_value(&hovers[0]).unwrap()), &shape(&serde_json::to_value(&hovers[1]).unwrap()));
        let feedback = shape(&serde_json::to_value(crate::Feedback { id: Uuid::new_v4(), ok: true, text: "✓".into() }).unwrap());
        let contract = json!({
            "uiState": ui,
            "overlay": shape(&serde_json::to_value(&frame).unwrap()),
            "hover": hover,
            "feedback": feedback,
            "netEntry": shape(&serde_json::to_value(NetEntry { at: Utc::now(), method: "GET".into(), host: "api.github.com".into(), path: "/".into(), status: 200, reason: "scheduled".into() }).unwrap()),
        });
        std::fs::remove_dir_all(&dir).unwrap();
        let text = serde_json::to_string_pretty(&contract).unwrap() + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract.json");
        if std::env::var_os("UPDATE_CONTRACT").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        let saved = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(saved == text, "the UI contract changed: run `UPDATE_CONTRACT=1 cargo test contract` and update src/ui/api.ts and tests/mock.ts to match");
    }
}
