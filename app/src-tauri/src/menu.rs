//! Menu-bar menu (§7.5) and the per-pet menu shared with the overlay's ⌥-click (§7.3).
use strays_core::chrono::{Duration, Local, Utc};
use strays_core::clock::Time;
use strays_core::pet::{Pet, Species, StateKind};
use strays_core::uuid::Uuid;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

use crate::state::{Action, App, Snooze, mood_word};

type R<T> = tauri::Result<T>;

fn item<Rt: Runtime>(h: &AppHandle<Rt>, id: &str, text: &str) -> R<MenuItemKind<Rt>> {
    Ok(MenuItemKind::MenuItem(MenuItem::with_id(h, id, text, true, None::<&str>)?))
}

fn label<Rt: Runtime>(h: &AppHandle<Rt>, text: &str) -> R<MenuItemKind<Rt>> {
    Ok(MenuItemKind::MenuItem(MenuItem::new(h, text, false, None::<&str>)?))
}

fn sub<Rt: Runtime>(h: &AppHandle<Rt>, text: &str, items: Vec<MenuItemKind<Rt>>) -> R<MenuItemKind<Rt>> {
    let refs: Vec<&dyn IsMenuItem<Rt>> = items.iter().map(|i| i as &dyn IsMenuItem<Rt>).collect();
    Ok(MenuItemKind::Submenu(Submenu::with_items(h, text, true, &refs)?))
}

fn sep<Rt: Runtime>(h: &AppHandle<Rt>) -> R<MenuItemKind<Rt>> {
    Ok(MenuItemKind::Predefined(PredefinedMenuItem::separator(h)?))
}

pub fn species_icon(s: Species) -> &'static str {
    match s {
        Species::Cat => "🐈",
        Species::Dog => "🐕",
        Species::Hamster => "🐹",
        Species::Parrot => "🦜",
    }
}

fn ago(t: Time, now: Time) -> String {
    let d = now - t;
    match d.num_minutes() {
        m if m < 60 => format!("{}m ago", m.max(0)),
        m if m < 48 * 60 => format!("{}h ago", m / 60),
        m => format!("{}d ago", m / (24 * 60)),
    }
}

/// Every action that applies to the pet in its current state.
pub fn pet_entries<Rt: Runtime>(h: &AppHandle<Rt>, app: &App, p: &Pet, now: Time) -> R<Vec<MenuItemKind<Rt>>> {
    let id = |a: &str| format!("pet/{}/{a}", p.id);
    let repo = p.project.github.as_ref().map(|g| g.full_name.clone()).unwrap_or_else(|| "local folder".into());
    let mut v = vec![label(h, &format!("{repo} · last activity {}", ago(p.activity.last, now)))?];
    let confirm = |what: &str, yes: &str, action: &str| -> R<MenuItemKind<Rt>> { sub(h, what, vec![item(h, &id(action), yes)?]) };
    if p.missing_since.is_some() && !matches!(p.state.kind, StateKind::LetGo | StateKind::Graduated) {
        v.push(label(h, "❓ GitHub can't find this repo. Deleted, or can't your token see it?")?);
        v.push(item(h, &id("letgo"), "It was deleted: let go")?);
        v.push(item(h, "setup", "Check the token…")?);
        v.push(item(h, &id("sync"), "Look again")?);
        v.push(sep(h)?);
    }
    match p.state.kind {
        StateKind::Active => {
            v.push(sub(
                h,
                "Snooze",
                vec![
                    item(h, &id("snooze-1h"), "For 1 hour")?,
                    item(h, &id("snooze-4h"), "For 4 hours")?,
                    item(h, &id("snooze-tomorrow"), "Until tomorrow")?,
                    item(h, &id("snooze-3d"), "For 3 days")?,
                    item(h, &id("snooze-wake"), "Until I wake it")?,
                ],
            )?);
            v.push(item(h, &id("sync"), "Sync now")?);
            v.push(item(h, &id("release"), "Mark release")?);
            v.push(item(h, &id("pause"), "Pause")?);
            v.push(item(h, &id("hide-today"), "Hide for today")?);
            v.push(confirm("Graduate…", "Yes, this project is finished", "graduate")?);
            v.push(confirm("Let go…", "Yes, stop tracking it", "letgo")?);
        }
        StateKind::Snoozed => {
            v.push(item(h, &id("wake"), "Wake now")?);
            v.push(item(h, &id("pause"), "Pause")?);
            v.push(confirm("Let go…", "Yes, stop tracking it", "letgo")?);
        }
        StateKind::Paused => {
            v.push(item(h, &id("resume"), "Resume")?);
            v.push(confirm("Let go…", "Yes, stop tracking it", "letgo")?);
        }
        StateKind::Holiday => {
            let back = p.state.until.map(|t| t.with_timezone(&Local).format("%a %-d %b").to_string()).unwrap_or_default();
            v.push(label(h, &format!("Back {back}"))?);
        }
        StateKind::Shelter => {
            let text = if app.slot_free() { "Adopt" } else { "Adopt (no free slot)" };
            v.push(MenuItemKind::MenuItem(MenuItem::with_id(h, id("adopt"), text, app.slot_free(), None::<&str>)?));
            v.push(confirm("Let go…", "Yes, stop tracking it", "letgo")?);
        }
        StateKind::LetGo => v.push(item(h, &id("undo"), "Bring back")?),
        StateKind::Graduated => {}
    }
    Ok(v)
}

fn pet_title(app: &App, p: &Pet, now: Time) -> String {
    let mood = if matches!(p.state.kind, StateKind::Active) { format!(" · {}", mood_word(app.mood(p, now))) } else { String::new() };
    let worn = match (p.state.kind, p.accessory.as_deref()) {
        (StateKind::Graduated, Some(a)) => format!(" · {}", accessory_label(a)),
        _ => String::new(),
    };
    let confused = if p.missing_since.is_some() && !matches!(p.state.kind, StateKind::LetGo | StateKind::Graduated) { " ❓" } else { "" };
    format!("{} {}{mood}{worn}{confused}", species_icon(p.species), p.name)
}

/// How the Graduates list names the accessory a pet earned.
fn accessory_label(a: &str) -> &'static str {
    match a {
        "cap" => "🎓 graduation cap",
        "scarf" => "🧣 scarf",
        "bow" => "🎀 bow",
        "star" => "⭐ star badge",
        "crown" => "🌸 flower crown",
        _ => "🎁 reward",
    }
}

pub fn status_line(app: &App) -> String {
    if let Some(e) = &app.sync_error {
        return format!("⚠ {e}");
    }
    match app.sync.last_successful_check {
        Some(t) => format!("Last synced {}", t.with_timezone(&Local).format("%H:%M")),
        None => "Not synced yet".into(),
    }
}

/// Days left on a connected token that expires within a week (§6.4: remind from 7 days before).
pub fn token_days_left(app: &App, now: Time) -> Option<i64> {
    let expires = app.sync.token_expires.or(app.account.as_ref()?.expires)?;
    let left = expires - now;
    // counted in calendar days, so "tomorrow" means tomorrow's date
    let days = (expires.with_timezone(&Local).date_naive() - now.with_timezone(&Local).date_naive()).num_days();
    (left > Duration::zero() && left <= Duration::days(7)).then_some(days)
}

fn token_reminder(days: i64) -> String {
    match days {
        0 => "⏳ GitHub token expires today. Paste a new one…".into(),
        1 => "⏳ GitHub token expires tomorrow. Paste a new one…".into(),
        n => format!("⏳ GitHub token expires in {n} days. Paste a new one…"),
    }
}

/// The badge next to the menu-bar paw icon: hidden needy pets and a soon-expiring token, or a warning when syncing fails.
pub fn tray_title(app: &App) -> String {
    if app.sync_error.is_some() || !app.issues.is_empty() {
        return "⚠".into();
    }
    let needy = (app.hidden_needy > 0).then(|| app.hidden_needy.to_string());
    let token = token_days_left(app, Utc::now()).map(|_| "⏳".to_string());
    [needy, token].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

pub fn build<Rt: Runtime>(h: &AppHandle<Rt>, app: &App) -> R<Menu<Rt>> {
    let now = Utc::now();
    let mut items = vec![label(h, &status_line(app))?, item(h, "sync", "Sync now")?];
    if let Some(days) = token_days_left(app, now) {
        items.push(item(h, "setup", &token_reminder(days))?);
    }
    for i in &app.issues {
        items.push(label(h, &format!("⚠ {}", i.text))?);
    }
    if !app.issues.is_empty() {
        items.push(item(h, "dismiss", "Dismiss warnings")?);
    }
    items.push(sep(h)?);
    let on_screen: Vec<&Pet> = app.visible.iter().filter_map(|id| app.pets.iter().find(|p| p.id == *id)).collect();
    type Keep<'a> = Box<dyn Fn(&Pet) -> bool + 'a>;
    let sections: [(&str, Keep); 8] = [
        ("🐾 On screen", Box::new(|p: &Pet| on_screen.iter().any(|v| v.id == p.id))),
        ("🛋 Resting", Box::new(|p: &Pet| p.state.kind == StateKind::Active && !on_screen.iter().any(|v| v.id == p.id))),
        ("🏠 Shelter", Box::new(|p: &Pet| p.state.kind == StateKind::Shelter)),
        ("💤 Snoozed", Box::new(|p: &Pet| p.state.kind == StateKind::Snoozed)),
        ("⏸ Paused", Box::new(|p: &Pet| p.state.kind == StateKind::Paused)),
        ("🏖 On holiday", Box::new(|p: &Pet| p.state.kind == StateKind::Holiday)),
        ("🎓 Graduates", Box::new(|p: &Pet| p.state.kind == StateKind::Graduated)),
        ("👋 Let go", Box::new(|p: &Pet| p.state.kind == StateKind::LetGo)),
    ];
    for (title, keep) in &sections {
        let pets: Vec<&Pet> = app.pets.iter().filter(|p| keep(p)).collect();
        if pets.is_empty() {
            continue;
        }
        items.push(label(h, &format!("{title} ({})", pets.len()))?);
        for p in pets {
            items.push(sub(h, &pet_title(app, p, now), pet_entries(h, app, p, now)?)?);
        }
    }
    if !app.pets.is_empty() {
        items.push(sep(h)?);
    }
    let repo_choices = |r: &strays_core::github::Repo| -> R<Vec<MenuItemKind<Rt>>> {
        let id = |a: &str| format!("repo/{}/{a}", r.id);
        Ok(vec![
            item(h, &id("cat"), "Adopt as a cat")?,
            item(h, &id("dog"), "Adopt as a dog")?,
            item(h, &id("hamster"), "Adopt as a hamster")?,
            item(h, &id("parrot"), "Adopt as a parrot")?,
            item(h, &id("shelter"), "Add to the shelter")?,
        ])
    };
    if !app.new_projects.is_empty() || !app.new_folders.is_empty() {
        let mut list = Vec::new();
        for f in &app.new_folders {
            // by path, not position: the list can change between building the menu and the click
            let choices = vec![item(h, &format!("newfolder/adopt/{f}"), "Adopt…")?, item(h, &format!("newfolder/ignore/{f}"), "Ignore")?];
            list.push(sub(h, &format!("📁 {}", crate::state::tilde(f)), choices)?);
        }
        for r in &app.new_projects {
            let mut choices = repo_choices(r)?;
            choices.push(item(h, &format!("repo/{}/ignore", r.id), "Ignore")?);
            list.push(sub(h, &r.full_name, choices)?);
        }
        items.push(sub(h, &format!("✨ New projects ({})", app.new_projects.len() + app.new_folders.len()), list)?);
    }
    let untracked: Vec<_> = app.repos.iter().filter(|r| !app.tracked(r.id) && !r.archived).take(20).collect();
    if !untracked.is_empty() {
        let mut list = Vec::new();
        for r in untracked {
            let when = r.pushed_at.map(|t| ago(t, now)).unwrap_or_else(|| "never pushed".into());
            list.push(sub(h, &format!("{} · {when}", r.full_name), repo_choices(r)?)?);
        }
        items.push(sub(h, "Adopt a project", list)?);
    }
    items.push(sep(h)?);
    items.push(MenuItemKind::Check(CheckMenuItem::with_id(h, "presentation", "Presentation mode", true, app.flags.presentation, None::<&str>)?));
    items.push(MenuItemKind::Check(CheckMenuItem::with_id(h, "vacation", "Vacation mode", true, app.flags.vacation, None::<&str>)?));
    items.push(MenuItemKind::Check(CheckMenuItem::with_id(h, "hide-all", "Hide all pets", true, app.flags.hide_all, None::<&str>)?));
    if app.pets.iter().any(|p| p.home.is_some()) {
        items.push(item(h, "line-up", "Put pets back in line")?);
    }
    items.push(sep(h)?);
    items.push(item(h, "adopt", "Adopt a pet…")?);
    if app.account.is_none() {
        items.push(item(h, "setup", "Connect GitHub…")?);
    }
    if let Some(r) = &app.sync.update {
        items.push(item(h, "update", &format!("⬆ Strays {} is out…", r.tag_name))?);
    }
    items.push(item(h, "settings", "Settings…")?);
    items.push(sep(h)?);
    items.push(item(h, "quit", "Quit Strays")?);
    let refs: Vec<&dyn IsMenuItem<Rt>> = items.iter().map(|i| i as &dyn IsMenuItem<Rt>).collect();
    Menu::with_items(h, &refs)
}

pub enum Event {
    Pet(Uuid, Action),
    SyncPet(Uuid),
    Repo(u64, Option<Species>),
    IgnoreRepo(u64),
    Sync,
    Presentation,
    Vacation,
    HideAll,
    LineUp,
    NewFolder(String, bool),
    Settings,
    Update,
    Setup,
    AdoptWindow,
    Dismiss,
    Quit,
}

/// What a click needs from the shell once `apply` has changed the state.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Done,
    Sync(Option<Uuid>),
    Open(Window),
    Quit,
}

#[derive(Debug, PartialEq)]
pub enum Window {
    Settings(&'static str),
    Setup,
    /// The adoption window's query, e.g. `folder=…`.
    Adopt(String),
}

/// Applies a menu click to the state. Success shows in the menu itself; a refusal ("All adoption slots
/// are full") is shown at the menu's top until the next action succeeds.
pub fn apply(app: &mut App, event: Event, now: Time) -> Outcome {
    let result = match event {
        Event::Pet(pet, action) => app.act(pet, action, now).map(Some),
        Event::SyncPet(pet) => return Outcome::Sync(Some(pet)),
        Event::Sync => return Outcome::Sync(None),
        Event::Repo(repo, species) => app.adopt_repo(repo, species, now).map(Some),
        Event::IgnoreRepo(repo) => {
            app.ignore_new(repo);
            Ok(None)
        }
        Event::Presentation => {
            app.flags.presentation = !app.flags.presentation;
            Ok(None)
        }
        Event::Vacation => {
            let on = !app.flags.vacation;
            app.set_vacation(on, now);
            Ok(None)
        }
        Event::HideAll => {
            app.flags.hide_all = !app.flags.hide_all;
            Ok(None)
        }
        Event::LineUp => {
            app.clear_homes();
            Ok(None)
        }
        // the menu may be older than the list: a folder already decided on is left alone
        Event::NewFolder(path, _) if !app.new_folders.contains(&path) => Ok(None),
        Event::NewFolder(path, true) => return Outcome::Open(Window::Adopt(format!("folder={}", crate::windows::encode(&path)))),
        Event::NewFolder(path, false) => {
            app.ignore_folder(&path);
            Ok(None)
        }
        Event::Settings => return Outcome::Open(Window::Settings("pets")),
        Event::Update => return Outcome::Open(Window::Settings("about")),
        Event::Setup => return Outcome::Open(Window::Setup),
        Event::AdoptWindow => return Outcome::Open(Window::Adopt(String::new())),
        Event::Dismiss => {
            app.issues.clear();
            Ok(None)
        }
        Event::Quit => {
            app.save_all();
            return Outcome::Quit;
        }
    };
    match result {
        Ok(Some(msg)) => {
            #[cfg(debug_assertions)]
            println!("ACTION {msg}");
            app.resolve("action");
        }
        Err(msg) => {
            #[cfg(debug_assertions)]
            println!("ACTION {msg}");
            app.warn("action", msg);
        }
        Ok(None) => {}
    }
    Outcome::Done
}

pub fn parse(id: &str) -> Option<Event> {
    if let Some((choice, path)) = id.strip_prefix("newfolder/").and_then(|r| r.split_once('/')) {
        return Some(Event::NewFolder(path.to_string(), choice == "adopt"));
    }
    let parts: Vec<&str> = id.split('/').collect();
    Some(match parts.as_slice() {
        ["sync"] => Event::Sync,
        ["presentation"] => Event::Presentation,
        ["vacation"] => Event::Vacation,
        ["hide-all"] => Event::HideAll,
        ["line-up"] => Event::LineUp,
        ["quit"] => Event::Quit,
        ["settings"] => Event::Settings,
        ["update"] => Event::Update,
        ["setup"] => Event::Setup,
        ["adopt"] => Event::AdoptWindow,
        ["dismiss"] => Event::Dismiss,
        ["pet", pet, action] => {
            let pet: Uuid = pet.parse().ok()?;
            let action = match *action {
                "sync" => return Some(Event::SyncPet(pet)),
                "snooze-1h" => Action::Snooze(Snooze::Hours(1)),
                "snooze-4h" => Action::Snooze(Snooze::Hours(4)),
                "snooze-tomorrow" | "hide-today" => Action::Snooze(Snooze::Tomorrow),
                "snooze-3d" => Action::Snooze(Snooze::Days(3)),
                "snooze-wake" => Action::Snooze(Snooze::UntilWake),
                "wake" => Action::Wake,
                "pause" => Action::Pause,
                "resume" => Action::Resume,
                "release" => Action::Release,
                "graduate" => Action::Graduate,
                "letgo" => Action::LetGo,
                "undo" => Action::UndoLetGo,
                "adopt" => Action::Adopt,
                _ => return None,
            };
            Event::Pet(pet, action)
        }
        ["repo", repo, choice] => {
            let repo: u64 = repo.parse().ok()?;
            match *choice {
                "ignore" => Event::IgnoreRepo(repo),
                "shelter" => Event::Repo(repo, None),
                "cat" => Event::Repo(repo, Some(Species::Cat)),
                "dog" => Event::Repo(repo, Some(Species::Dog)),
                "hamster" => Event::Repo(repo, Some(Species::Hamster)),
                "parrot" => Event::Repo(repo, Some(Species::Parrot)),
                _ => return None,
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        let u = Uuid::new_v4();
        assert!(matches!(parse(&format!("pet/{u}/snooze-tomorrow")), Some(Event::Pet(p, Action::Snooze(Snooze::Tomorrow))) if p == u));
        assert!(matches!(parse(&format!("pet/{u}/sync")), Some(Event::SyncPet(p)) if p == u));
        assert!(matches!(parse("repo/42/parrot"), Some(Event::Repo(42, Some(Species::Parrot)))));
        assert!(matches!(parse("repo/42/shelter"), Some(Event::Repo(42, None))));
        assert!(parse("pet/not-a-uuid/wake").is_none());
        assert!(parse("pet/x").is_none());
        assert!(matches!(parse("quit"), Some(Event::Quit)));
        assert!(matches!(parse("newfolder/adopt//Users/me/Code/a b"), Some(Event::NewFolder(p, true)) if p == "/Users/me/Code/a b"));
    }

    /// Every id the menu builds must parse back, or its click does nothing: checked from this file's source.
    #[test]
    fn every_id_the_menu_builds_parses() {
        let src = include_str!("menu.rs");
        let src = &src[..src.find("#[cfg(test)]").unwrap()];
        let quoted = |after: &str| -> Vec<String> { src.match_indices(after).map(|(i, _)| src[i + after.len()..].split('"').next().unwrap().to_string()).collect() };
        let pet = Uuid::new_v4();
        // pet actions are built in pet_entries; repo choices use their own `id` and are checked below
        let entries = &src[src.find("pub fn pet_entries").unwrap()..src.find("fn pet_title").unwrap()];
        let mut actions: Vec<String> = entries.match_indices("id(\"").map(|(i, _)| entries[i + 4..].split('"').next().unwrap().to_string()).collect();
        // confirm("Let go…", "Yes, …", "letgo"): the action is the third string
        actions.extend(entries.match_indices("confirm(\"").filter_map(|(i, _)| entries[i..].split('"').nth(5).map(String::from)));
        assert!(actions.len() > 15 && actions.contains(&"graduate".to_string()) && actions.contains(&"adopt".to_string()), "{actions:?}");
        for a in &actions {
            assert!(parse(&format!("pet/{pet}/{a}")).is_some(), "pet action {a}");
        }
        let plain = quoted("item(h, \"");
        assert!(plain.len() > 4, "{plain:?}");
        for id in plain.iter().chain(&quoted("CheckMenuItem::with_id(h, \"")) {
            assert!(parse(id).is_some(), "menu id {id}");
        }
        for r in ["cat", "dog", "hamster", "parrot", "shelter", "ignore"] {
            assert!(parse(&format!("repo/1/{r}")).is_some());
        }
    }

    fn pet(app: &mut App, now: Time) -> Uuid {
        use strays_core::pet::{Activity, ActivitySource, Project};
        let mut p = Pet::new(now, "Miso", Species::Cat, Project::default(), Activity { last: now, source: ActivitySource::LocalCommit }, strays_core::mood::Pace::Normal);
        p.adopt(now).unwrap();
        let id = p.id;
        app.pets.push(p);
        id
    }

    #[test]
    fn clicks_change_state_refusals_are_shown_and_windows_are_asked_for() {
        let dir = std::env::temp_dir().join(format!("strays-menu-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        let now = Utc::now();
        let id = pet(&mut app, now);
        assert_eq!(apply(&mut app, Event::Pet(id, Action::Snooze(Snooze::Hours(1))), now), Outcome::Done);
        assert_eq!(app.pets[0].state.kind, StateKind::Snoozed);
        // waking an awake pet is refused: the reason shows at the menu's top, the next success clears it
        apply(&mut app, Event::Pet(id, Action::Wake), now);
        assert_eq!(apply(&mut app, Event::Pet(id, Action::Wake), now), Outcome::Done);
        assert!(app.issues.iter().any(|i| i.key == "action"), "{:?}", app.issues);
        apply(&mut app, Event::Pet(id, Action::Pause), now);
        assert!(!app.issues.iter().any(|i| i.key == "action"));
        assert_eq!(apply(&mut app, Event::SyncPet(id), now), Outcome::Sync(Some(id)));
        assert_eq!(apply(&mut app, Event::Presentation, now), Outcome::Done);
        assert!(app.flags.presentation);
        // a folder no longer on the list is left alone; one on it opens adoption for it
        assert_eq!(apply(&mut app, Event::NewFolder("/x".into(), true), now), Outcome::Done);
        app.new_folders.push("/Users/me/Code/new app".into());
        assert_eq!(apply(&mut app, Event::NewFolder("/Users/me/Code/new app".into(), true), now), Outcome::Open(Window::Adopt("folder=/Users/me/Code/new%20app".into())));
        apply(&mut app, Event::NewFolder("/Users/me/Code/new app".into(), false), now);
        assert!(app.new_folders.is_empty());
        app.warn("x", "y");
        apply(&mut app, Event::Dismiss, now);
        assert!(app.issues.is_empty());
        assert_eq!(apply(&mut app, Event::Settings, now), Outcome::Open(Window::Settings("pets")));
        assert_eq!(apply(&mut app, Event::Quit, now), Outcome::Quit);
        assert!(dir.join("pets").join(format!("{id}.json")).exists(), "quitting saves");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn token_reminder_shows_in_the_last_week_only() {
        let dir = std::env::temp_dir().join(format!("strays-exp-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        let now = Utc::now();
        let at = |d: i64| Some(crate::state::Account { login: "me".into(), expires: Some(now + Duration::days(d) + Duration::hours(1)), repo_count: 1 });
        app.account = at(8);
        assert_eq!(token_days_left(&app, now), None);
        app.account = at(6);
        assert!(matches!(token_days_left(&app, now), Some(6 | 7)));
        assert_eq!(tray_title(&app), "⏳");
        assert_eq!(token_reminder(1), "⏳ GitHub token expires tomorrow. Paste a new one…");
        app.account = at(-1);
        assert_eq!(token_days_left(&app, now), None, "expired: the 401 warning takes over");
        app.account = None;
        assert_eq!(token_days_left(&app, now), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
