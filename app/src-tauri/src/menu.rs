//! Menu-bar menu (§7.5) and the per-pet menu shared with the overlay's ⌥-click (§7.3).
use strays_core::chrono::{Local, Utc};
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
    format!("{} {}{mood}", species_icon(p.species), p.name)
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

/// The badge next to the menu-bar icon: hidden needy pets, or a warning when syncing fails.
pub fn tray_title(app: &App) -> String {
    match (app.sync_error.is_some(), app.hidden_needy) {
        (true, _) => "🐾⚠".into(),
        (false, 0) => "🐾".into(),
        (false, n) => format!("🐾{n}"),
    }
}

pub fn build<Rt: Runtime>(h: &AppHandle<Rt>, app: &App) -> R<Menu<Rt>> {
    let now = Utc::now();
    let mut items = vec![label(h, &status_line(app))?, item(h, "sync", "Sync now")?];
    if app.load_problems > 0 {
        items.push(label(h, &format!("⚠ {} pet file(s) couldn't be read and were left untouched", app.load_problems))?);
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
        for (i, f) in app.new_folders.iter().enumerate() {
            let choices = vec![item(h, &format!("newfolder/{i}/adopt"), "Adopt…")?, item(h, &format!("newfolder/{i}/ignore"), "Ignore")?];
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
    items.push(sep(h)?);
    items.push(item(h, "adopt", "Adopt a pet…")?);
    if app.account.is_none() {
        items.push(item(h, "setup", "Connect GitHub…")?);
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
    NewFolder(usize, bool),
    Settings,
    Setup,
    AdoptWindow,
    Quit,
}

pub fn parse(id: &str) -> Option<Event> {
    let parts: Vec<&str> = id.split('/').collect();
    Some(match parts.as_slice() {
        ["sync"] => Event::Sync,
        ["presentation"] => Event::Presentation,
        ["vacation"] => Event::Vacation,
        ["hide-all"] => Event::HideAll,
        ["quit"] => Event::Quit,
        ["settings"] => Event::Settings,
        ["newfolder", i, choice] => Event::NewFolder(i.parse().ok()?, *choice == "adopt"),
        ["setup"] => Event::Setup,
        ["adopt"] => Event::AdoptWindow,
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
    }
}
