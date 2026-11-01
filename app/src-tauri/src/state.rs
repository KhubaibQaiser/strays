//! App state around the core: loaded pets, settings, sync state, toggles and who is on screen.
use std::collections::HashMap;
use std::path::PathBuf;

use strays_core::chrono::{Duration, Local, NaiveTime, TimeZone};
use serde::Serialize;
use strays_core::clock::Time;
use strays_core::display;
use strays_core::github::{Repo, SyncState};
use strays_core::mood::Mood;
use strays_core::pet::{self, Activity, ActivitySource, GithubRef, Pet, Project, ReleaseOutcome, Species, StateKind};
use strays_core::settings::Settings;
use strays_core::store::Store;
use strays_core::uuid::Uuid;

const SYNC_FILE: &str = "sync-state.json";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flags {
    pub presentation: bool,
    pub vacation: bool,
    pub hide_all: bool,
}

pub struct App {
    store: Store,
    pub settings: Settings,
    pub pets: Vec<Pet>,
    pub sync: SyncState,
    pub flags: Flags,
    /// Every repo the token sees, newest push first, for "Adopt a project".
    pub repos: Vec<Repo>,
    /// New repos waiting for the user's decision (§5.9).
    pub new_projects: Vec<Repo>,
    pub visible: Vec<Uuid>,
    pub hidden_needy: usize,
    last_shown: HashMap<Uuid, Time>,
    rotation_started: Option<Time>,
    pub sync_error: Option<String>,
    pub load_problems: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Snooze {
    Hours(i64),
    Tomorrow,
    Days(i64),
    UntilWake,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Adopt,
    Snooze(Snooze),
    Wake,
    Pause,
    Resume,
    Release,
    Graduate,
    LetGo,
    UndoLetGo,
}

/// What the overlay needs to draw one pet. Contains no token and no paths.
#[derive(Clone, Debug, Serialize)]
pub struct PetView {
    pub id: Uuid,
    pub species: Species,
    pub colors: std::collections::BTreeMap<String, String>,
    pub label: String,
    pub mood: Mood,
}

pub fn mood_word(m: Mood) -> &'static str {
    match m {
        Mood::Joyful => "Joyful",
        Mood::Content => "Content",
        Mood::Peckish => "Peckish",
        Mood::Hungry => "Hungry",
        Mood::Gloomy => "Gloomy",
    }
}

/// Coat colours offered at adoption, per Rive view-model property (`art/cat`). One is picked at random.
const LOOKS: [[&str; 4]; 4] = [
    ["#EFE1CC", "#D9C4A6", "#FFF8EE", "#7A5C46"],
    ["#34343A", "#26262B", "#55555E", "#A9A9B6"],
    ["#E8A15C", "#CB8240", "#FBE3C6", "#7A4520"],
    ["#B9C0CC", "#9AA2B0", "#E3E7EE", "#4E5563"],
];

impl App {
    pub fn load(dir: PathBuf) -> Result<Self, String> {
        let store = Store::open(dir).map_err(|e| format!("{e:?}"))?;
        let settings = store.load_settings().unwrap_or_else(|e| {
            eprintln!("settings unreadable, using defaults (file kept): {e:?}");
            Settings::default()
        });
        let (pets, problems) = store.load_pets().map_err(|e| format!("{e:?}"))?;
        for p in &problems {
            eprintln!("pet file skipped: {p:?}");
        }
        let sync: SyncState = store.load(SYNC_FILE).unwrap_or_default();
        Ok(Self {
            store,
            settings,
            pets,
            sync,
            flags: Flags::default(),
            repos: Vec::new(),
            new_projects: Vec::new(),
            visible: Vec::new(),
            hidden_needy: 0,
            last_shown: HashMap::new(),
            rotation_started: None,
            sync_error: None,
            load_problems: problems.len(),
        })
    }

    pub fn save_pet(&self, id: Uuid) {
        if let Some(p) = self.pets.iter().find(|p| p.id == id)
            && let Err(e) = self.store.save_pet(p)
        {
            eprintln!("could not save pet: {e:?}");
        }
    }

    pub fn save_all(&self) {
        for p in &self.pets {
            self.save_pet(p.id);
        }
        if let Err(e) = self.store.save(SYNC_FILE, &self.sync) {
            eprintln!("could not save sync state: {e:?}");
        }
    }

    pub fn mood(&self, p: &Pet, now: Time) -> Mood {
        p.mood(now, self.sync.last_successful_check, &self.settings, &Local)
    }

    pub fn tracked(&self, repo_id: u64) -> bool {
        self.pets.iter().any(|p| p.project.github.as_ref().is_some_and(|g| g.id == repo_id))
    }

    pub fn slot_free(&self) -> bool {
        pet::slots_used(&self.pets) < self.settings.slots as usize
    }

    /// Ends due snoozes, holidays and scheduled resumes. Returns whether anything changed.
    pub fn tick(&mut self, now: Time) -> bool {
        let mut changed = Vec::new();
        for p in &mut self.pets {
            if p.tick(now) {
                changed.push(p.id);
            }
        }
        changed.iter().for_each(|&id| self.save_pet(id));
        !changed.is_empty()
    }

    pub fn next_deadline(&self) -> Option<Time> {
        self.pets.iter().filter_map(Pet::next_deadline).min()
    }

    /// Re-picks the on-screen pets (§5.7). The set rotates every `rotation_hours`.
    pub fn refresh_visible(&mut self, now: Time) {
        let rotate = self.rotation_started.is_none_or(|t| now - t >= Duration::hours(self.settings.rotation_hours.into()));
        if rotate {
            for id in &self.visible {
                self.last_shown.insert(*id, now);
            }
            self.rotation_started = Some(now);
        }
        let moods: HashMap<Uuid, Mood> = self.pets.iter().map(|p| (p.id, self.mood(p, now))).collect();
        let sel = display::choose(&self.pets, &moods, &self.last_shown, self.settings.max_visible as usize);
        self.hidden_needy = sel.hidden_needy;
        self.visible = if self.flags.hide_all { Vec::new() } else { sel.visible };
    }

    pub fn views(&self, now: Time) -> Vec<PetView> {
        self.visible
            .iter()
            .filter_map(|id| self.pets.iter().find(|p| p.id == *id))
            .map(|p| {
                let mood = self.mood(p, now);
                let repo = p.project.github.as_ref().map(|g| g.full_name.clone()).or_else(|| p.project.local.as_ref().map(|l| l.display_path.clone()));
                let label = match repo.filter(|_| !self.flags.presentation) {
                    Some(r) => format!("{} · {r} · {}", p.name, mood_word(mood)),
                    None => format!("{} · {}", p.name, mood_word(mood)),
                };
                PetView { id: p.id, species: p.species, colors: p.colors.clone(), label, mood }
            })
            .collect()
    }

    /// Creates a pet for a GitHub repo. It's adopted straight away if a slot is free, else it waits in the shelter.
    pub fn adopt_repo(&mut self, repo_id: u64, species: Option<Species>, now: Time) -> Result<String, String> {
        if self.tracked(repo_id) {
            return Err("Already tracked".into());
        }
        let repo = self.repos.iter().chain(&self.new_projects).find(|r| r.id == repo_id).cloned().ok_or("Unknown repo")?;
        let gh = GithubRef { id: repo.id, full_name: repo.full_name.clone(), account: repo.owner.login.clone() };
        let last = repo.pushed_at.unwrap_or(repo.created_at);
        let into_shelter = species.is_none();
        let species = species.unwrap_or(Species::Cat);
        let mut p = Pet::new(now, &repo.name, species, Project { github: Some(gh), local: None }, Activity { last, source: ActivitySource::Github }, self.settings.default_pace);
        let look = LOOKS[(p.id.as_u128() % LOOKS.len() as u128) as usize];
        for (name, hex) in ["coatBase", "coatShade", "coatHighlight", "outline"].iter().zip(look) {
            p.colors.insert((*name).into(), hex.into());
        }
        let message = if !into_shelter && self.slot_free() {
            p.adopt(now).expect("new pets start in the shelter");
            format!("Adopted {}", repo.name)
        } else {
            format!("{} is in the shelter", repo.name)
        };
        let id = p.id;
        self.pets.push(p);
        self.new_projects.retain(|r| r.id != repo_id);
        self.save_pet(id);
        Ok(message)
    }

    pub fn ignore_new(&mut self, repo_id: u64) {
        self.new_projects.retain(|r| r.id != repo_id);
    }

    pub fn act(&mut self, id: Uuid, action: Action, now: Time) -> Result<String, String> {
        let slot_free = self.slot_free();
        let settings = self.settings.clone();
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        let name = p.name.clone();
        let refused = |e: pet::Refused| format!("{e:?}");
        let msg = match action {
            Action::Adopt if !slot_free => return Err("All adoption slots are full".into()),
            Action::Adopt => p.adopt(now).map(|_| format!("{name} moved in")).map_err(refused)?,
            Action::Snooze(s) => {
                p.snooze(now, snooze_until(s, now)).map_err(refused)?;
                format!("{name} is asleep")
            }
            Action::Wake => p.wake(now).map(|_| format!("{name} woke up")).map_err(refused)?,
            Action::Pause => p.pause(now, None).map(|_| format!("{name} is paused")).map_err(refused)?,
            Action::Resume => p.resume(now).map(|_| format!("{name} is back")).map_err(refused)?,
            Action::Release => match p.mark_release(now, None, &settings).map_err(refused)? {
                ReleaseOutcome::Holiday { .. } => format!("Shipped! {name} is taking a little holiday"),
                ReleaseOutcome::CelebrationOnly { reason } => format!("Shipped! ({reason}, so no holiday)"),
            },
            Action::Graduate => p.graduate(now, "Marked finished").map(|_| format!("{name} graduated 🎓")).map_err(refused)?,
            Action::LetGo => p.let_go(now, "Let go").map(|_| format!("Let {name} go")).map_err(refused)?,
            Action::UndoLetGo => p.undo_let_go(now, &settings).map(|_| format!("{name} is back in the shelter")).map_err(refused)?,
        };
        self.save_pet(id);
        Ok(msg)
    }

    /// Vacation mode pauses everyone (§5.10) and resumes exactly the pets it paused.
    pub fn set_vacation(&mut self, on: bool, now: Time) {
        self.flags.vacation = on;
        for p in &mut self.pets {
            if on && matches!(p.state.kind, StateKind::Active | StateKind::Snoozed) {
                p.pause(now, None).expect("active or snoozed");
                p.state.reason = "vacation".into();
            } else if !on && p.state.kind == StateKind::Paused && p.state.reason == "vacation" {
                p.resume(now).expect("paused");
            }
        }
        self.save_all();
    }
}

/// "Until tomorrow" means 09:00 local the next day (§5.6).
pub fn snooze_until(s: Snooze, now: Time) -> Option<Time> {
    match s {
        Snooze::Hours(h) => Some(now + Duration::hours(h)),
        Snooze::Days(d) => Some(now + Duration::days(d)),
        Snooze::UntilWake => None,
        Snooze::Tomorrow => {
            let day = now.with_timezone(&Local).date_naive() + Duration::days(1);
            let nine = day.and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
            Local.from_local_datetime(&nine).earliest().map(|t| t.to_utc()).or(Some(now + Duration::hours(24)))
        }
    }
}
