//! Commands the windows call. The token goes in through `token_check` / `token_save` and never comes back out.
use serde::Serialize;
use strays_core::chrono::{Local, Timelike, Utc};
use strays_core::clock::Time;
use strays_core::github::{self, Github, NetEntry};
use strays_core::mood::Pace;
use strays_core::pet::{Goal, Pet, Species, StateKind};
use strays_core::settings::Settings;
use strays_core::uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::Shared;
use crate::state::{Account, AdoptRequest, App, mood_word};
use crate::sync::Cmd;
use crate::{menu, windows};

type Res<T> = Result<T, String>;

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
    on_screen: bool,
    releases: usize,
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
        on_screen: app.visible.contains(&p.id),
        releases: p.releases.len(),
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
}

#[tauri::command]
pub fn set_goal(id: Uuid, goal: Goal, shared: State<'_, Shared>) -> Res<()> {
    shared.app.lock().unwrap().set_goal(id, goal)?;
    changed(&shared);
    Ok(())
}

#[tauri::command]
pub fn set_pet(id: Uuid, name: String, pace: Pace, shared: State<'_, Shared>) -> Res<()> {
    shared.app.lock().unwrap().set_pet(id, name, pace)?;
    changed(&shared);
    Ok(())
}

#[tauri::command]
pub fn adopt(request: AdoptRequest, shared: State<'_, Shared>) -> Res<String> {
    let r = shared.app.lock().unwrap().adopt(request, Utc::now());
    changed(&shared);
    r
}

#[tauri::command]
pub fn ignore_repo(id: u64, shared: State<'_, Shared>) {
    shared.app.lock().unwrap().ignore_new(id);
    changed(&shared);
}

#[tauri::command]
pub fn ignore_folder(path: String, shared: State<'_, Shared>) {
    shared.app.lock().unwrap().ignore_folder(&path);
    changed(&shared);
}

#[tauri::command]
pub fn save_settings(settings: Settings, shared: State<'_, Shared>) -> Res<()> {
    let r = shared.app.lock().unwrap().save_settings(settings);
    changed(&shared);
    r
}

#[tauri::command]
pub fn set_flag(name: String, on: bool, shared: State<'_, Shared>) -> Res<()> {
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
}

/// Settings → Sound → Test: plays a greeting at the chosen volume, even with sound off.
#[tauri::command]
pub fn test_sound(shared: State<'_, Shared>) -> Res<()> {
    let volume = shared.app.lock().map_err(|_| "busy")?.settings.volume;
    shared.audio.play(crate::audio::Sound::Greeting(strays_core::pet::Species::Cat), volume);
    Ok(())
}

/// Settings → Privacy → Launch at login. macOS keeps the state (System Settings → Login Items), so nothing is saved here.
#[tauri::command]
pub fn set_login_item(on: bool, h: AppHandle) -> Res<crate::platform::LoginItem> {
    let state = crate::platform::set_login_item(on)?;
    emit_changed(&h);
    Ok(state)
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
}

/// Setup step 4: validates again, stores the token in the Keychain and starts the first sync.
#[tauri::command]
pub async fn token_save(token: String, shared: State<'_, Shared>) -> Res<()> {
    let app = shared.app.clone();
    let tx = shared.tx.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let gh = client(&token)?;
        let v = gh.validate();
        let mut app = app.lock().unwrap();
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
}

/// Deletes the Keychain item straight away (§6.4). Pets keep their data and freeze.
#[tauri::command]
pub fn token_disconnect(shared: State<'_, Shared>) -> Res<()> {
    crate::token::delete()?;
    let mut app = shared.app.lock().unwrap();
    app.has_token = false;
    app.set_account(None);
    app.sync_error = Some("Not connected to GitHub".into());
    drop(app);
    changed(&shared);
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(h: AppHandle) -> Option<String> {
    let path = h.dialog().file().set_title("Choose a project folder").blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())?;
    // picking is the only moment the sandbox grants access; the bookmark keeps it across restarts
    #[cfg(target_os = "macos")]
    {
        let shared = h.state::<Shared>();
        let mut app = shared.app.lock().unwrap();
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
    let app = shared.app.lock().unwrap();
    let repo = app.pets.iter().find(|p| p.id == id).and_then(|p| p.project.github.clone()).ok_or("Not on GitHub")?;
    h.opener().open_url(format!("https://github.com/{}", repo.full_name), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal(id: Uuid, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    let app = shared.app.lock().unwrap();
    let path = app.pets.iter().find(|p| p.id == id).and_then(|p| p.project.local.clone()).map(|l| l.path).ok_or("No local folder")?;
    h.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Opens one of a fixed set of pages in the browser; the web view can't name arbitrary URLs.
#[tauri::command]
pub fn open_link(which: String, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    let url = match which.as_str() {
        "token-page" => "https://github.com/settings/personal-access-tokens/new".to_string(),
        // the page comes from GitHub's answer, so only this repo's release pages are opened
        "release" => shared.app.lock().unwrap().sync.update.as_ref().map(|r| r.html_url.clone())
            .filter(|u| u.starts_with(&format!("https://github.com/{}/releases/", strays_core::github::RELEASES_REPO)))
            .ok_or("No update to download")?,
        _ => return Err("Unknown link".into()),
    };
    h.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn netlog(shared: State<'_, Shared>) -> Vec<NetEntry> {
    let mut log = shared.app.lock().unwrap().netlog();
    log.reverse();
    log
}

#[tauri::command]
pub fn open_data_folder(shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    let dir = shared.app.lock().unwrap().dir.clone();
    h.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

/// Writes every pet, the settings and the sync state into one JSON file and reveals it. No token.
#[tauri::command]
pub fn export_data(shared: State<'_, Shared>, h: AppHandle) -> Res<String> {
    let app = shared.app.lock().unwrap();
    let export = serde_json::json!({ "pets": app.pets, "settings": app.settings, "sync": app.sync, "account": app.account });
    let path = app.dir.join(format!("strays-export-{}.json", Utc::now().format("%Y%m%d-%H%M%S")));
    std::fs::write(&path, serde_json::to_vec_pretty(&export).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    h.opener().reveal_item_in_dir(&path).map_err(|e| format!("Exported to {}, but Finder couldn't show it: {e}", path.display()))?;
    Ok(path.display().to_string())
}

/// §8.5 "Delete everything": the Keychain item, every data file and log, then quit.
#[tauri::command]
pub fn delete_everything(confirm: String, shared: State<'_, Shared>, h: AppHandle) -> Res<()> {
    if confirm != "DELETE" {
        return Err("Type DELETE to confirm".into());
    }
    let mut failed = Vec::new();
    if let Err(e) = crate::token::delete() {
        failed.push(e);
    }
    if crate::platform::login_item() != crate::platform::LoginItem::Off
        && let Err(e) = crate::platform::set_login_item(false)
    {
        failed.push(format!("the login item ({e})"));
    }
    let mut dirs = vec![shared.app.lock().unwrap().dir.clone()];
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
    if !failed.is_empty() {
        return Err(format!("Some things couldn't be deleted: {}. Try again, or remove them by hand.", failed.join("; ")));
    }
    h.exit(0);
    Ok(())
}

#[tauri::command]
pub fn open_window(kind: String, arg: Option<String>, h: AppHandle) -> Res<()> {
    let arg = arg.unwrap_or_default();
    match kind.as_str() {
        "settings" => windows::settings(&h, if arg.is_empty() { "pets" } else { &arg }),
        "setup" => windows::setup(&h),
        "adopt" => windows::adopt(&h, &arg),
        "welcome" => windows::welcome(&h),
        _ => return Err("Unknown window".into()),
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_window(label: String, h: AppHandle) -> Res<()> {
    match h.get_webview_window(&label) {
        Some(w) => w.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Minute of the local day, for the UI's quiet-hours preview.
#[tauri::command]
pub fn local_minute() -> u32 {
    let n = Local::now();
    n.hour() * 60 + n.minute()
}

/// Windows re-fetch `ui_state` when they hear this.
pub fn emit_changed(h: &AppHandle) {
    let _ = h.emit("state-changed", ());
}
