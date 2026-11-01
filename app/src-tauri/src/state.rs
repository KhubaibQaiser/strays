//! App state around the core: loaded pets, settings, sync state, toggles and who is on screen.
use std::collections::HashMap;
use std::path::PathBuf;

use strays_core::chrono::{Duration, Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use strays_core::clock::Time;
use strays_core::display;
use strays_core::github::{NetEntry, Repo, SyncState};
use strays_core::localgit;
use strays_core::mood::Pace;
use strays_core::pet::{Goal, LocalRef};
use strays_core::mood::Mood;
use strays_core::pet::{self, Activity, ActivitySource, GithubRef, Pet, Project, ReleaseOutcome, Species, StateKind};
use strays_core::settings::Settings;
use strays_core::store::Store;
use strays_core::uuid::Uuid;

const SYNC_FILE: &str = "sync-state.json";
const ACCOUNT_FILE: &str = "accounts.json";
const NETLOG_FILE: &str = "netlog.jsonl";
const NETLOG_KEEP: usize = 1000;
const FOLDERS_FILE: &str = "folders.json";
const NUDGE_FILE: &str = "nudges.json";

/// Git folders already seen inside watched folders, so only ones that appear later count as new (§5.9).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KnownFolders {
    roots: std::collections::BTreeSet<String>,
    repos: std::collections::BTreeSet<String>,
}

/// The connected GitHub account (no secrets; the token itself is in the Keychain).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub expires: Option<Time>,
    pub repo_count: u64,
}

/// Everything the adoption flow (§8.2) collects.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptRequest {
    pub repo_id: Option<u64>,
    pub folder: Option<String>,
    pub species: Species,
    pub colors: std::collections::BTreeMap<String, String>,
    pub name: String,
    pub goal: String,
    pub pace: Pace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flags {
    pub presentation: bool,
    pub vacation: bool,
    pub hide_all: bool,
}

pub struct App {
    store: Store,
    pub dir: PathBuf,
    pub account: Option<Account>,
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
    /// Git repos found directly inside the watched folders.
    pub local_repos: Vec<String>,
    /// Ones that appeared after their folder was first watched, waiting for the user's decision.
    pub new_folders: Vec<String>,
    known_folders: KnownFolders,
    nudge_log: strays_core::nudge::NudgeLog,
    pub load_problems: usize,
    /// Clips waiting to be played; `Eat` waits until its pet is on screen.
    pub cues: Vec<(Uuid, Cue)>,
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

/// Minutes after local midnight now, for quiet hours.
pub fn local_minute() -> u32 {
    use strays_core::chrono::Timelike;
    let t = Local::now();
    t.hour() * 60 + t.minute()
}

/// A one-off clip the overlay should play (spec §5.5, §5.6, §7.4). Matches the rig's `action` clips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cue {
    /// New activity: the bowl fills and the pet eats, the next time it's on screen.
    Eat,
    /// A release: celebrate, then leave on holiday if `leave`.
    Celebrate { leave: bool },
    /// Snoozed: fall asleep and fade out.
    FallAsleep,
    /// Snooze over: appear asleep, then wake with a stretch.
    Wake,
    /// Back from holiday: walk in with a postcard.
    Postcard,
    /// Just adopted: walk in and say hello.
    Hello,
    /// Graduated: put on the earned accessory (the rig's `accessory` number, 1-5), celebrate, then leave for good.
    Graduate { accessory: u8 },
}

impl Cue {
    /// The farewell for a pet that just graduated, wearing the accessory it earned.
    pub fn graduate(p: &Pet) -> Cue {
        let accessory = p.accessory.as_deref().and_then(|a| pet::ACCESSORIES.iter().position(|&x| x == a)).map_or(0, |i| i as u8 + 1);
        Cue::Graduate { accessory }
    }
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
        let store = Store::open(dir.clone()).map_err(|e| format!("{e:?}"))?;
        let settings = store.load_settings().unwrap_or_else(|e| {
            eprintln!("settings unreadable, using defaults (file kept): {e:?}");
            Settings::default()
        });
        let (pets, problems) = store.load_pets().map_err(|e| format!("{e:?}"))?;
        for p in &problems {
            eprintln!("pet file skipped: {p:?}");
        }
        let sync: SyncState = store.load(SYNC_FILE).unwrap_or_default();
        let account: Option<Account> = store.load(ACCOUNT_FILE).unwrap_or_default();
        let known_folders: KnownFolders = store.load(FOLDERS_FILE).unwrap_or_default();
        let nudge_log = store.load(NUDGE_FILE).unwrap_or_default();
        Ok(Self {
            store,
            dir,
            account,
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
            cues: Vec::new(),
            local_repos: Vec::new(),
            new_folders: Vec::new(),
            known_folders,
            nudge_log,
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
            let was = p.state.kind;
            if p.tick(now) {
                changed.push(p.id);
                match was {
                    StateKind::Snoozed => self.cues.push((p.id, Cue::Wake)),
                    StateKind::Holiday => self.cues.push((p.id, Cue::Postcard)),
                    _ => {}
                }
            }
        }
        changed.iter().for_each(|&id| self.save_pet(id));
        !changed.is_empty()
    }

    /// Cues for pets on screen now. `Eat` waits for its pet to appear; other cues are dropped if it isn't.
    /// The cue for a pet that is just leaving (release, snooze) arrives before the frame that removes it.
    pub fn take_cues(&mut self, was_visible: &[Uuid]) -> Vec<(Uuid, Cue)> {
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.cues).into_iter().partition(|(id, cue)| {
            let leaving = matches!(cue, Cue::Celebrate { .. } | Cue::FallAsleep | Cue::Graduate { .. });
            (if leaving { was_visible } else { &self.visible[..] }).contains(id)
        });
        self.cues = later.into_iter().filter(|(_, c)| *c == Cue::Eat).collect();
        let mut seen = std::collections::HashSet::new();
        now.into_iter().filter(|c| seen.insert(*c)).collect()
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
            self.cues.push((p.id, Cue::Hello));
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

    pub fn ignore_folder(&mut self, path: &str) {
        self.new_folders.retain(|f| f != path);
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
            Action::Adopt => {
                p.adopt(now).map_err(refused)?;
                self.cues.push((id, Cue::Hello));
                format!("{name} moved in")
            }
            Action::Snooze(s) => {
                p.snooze(now, snooze_until(s, now)).map_err(refused)?;
                self.cues.push((id, Cue::FallAsleep));
                format!("{name} is asleep")
            }
            Action::Wake => p.wake(now).map(|_| format!("{name} woke up")).map_err(refused)?,
            Action::Pause => p.pause(now, None).map(|_| format!("{name} is paused")).map_err(refused)?,
            Action::Resume => p.resume(now).map(|_| format!("{name} is back")).map_err(refused)?,
            Action::Release => match p.mark_release(now, None, &settings).map_err(refused)? {
                ReleaseOutcome::Holiday { .. } => {
                    self.cues.push((id, Cue::Celebrate { leave: true }));
                    format!("Shipped! {name} is taking a little holiday")
                }
                ReleaseOutcome::CelebrationOnly { reason } => {
                    self.cues.push((id, Cue::Celebrate { leave: false }));
                    format!("Shipped! ({reason}, so no holiday)")
                }
            },
            Action::Graduate => {
                p.graduate(now, "Marked finished").map_err(refused)?;
                self.cues.push((id, Cue::graduate(p)));
                format!("{name} graduated 🎓")
            }
            Action::LetGo => p.let_go(now, "Let go").map(|_| format!("Let {name} go")).map_err(refused)?,
            Action::UndoLetGo => p.undo_let_go(now, &settings).map(|_| format!("{name} is back in the shelter")).map_err(refused)?,
        };
        self.save_pet(id);
        Ok(msg)
    }

    pub fn tracked_folder(&self, path: &str) -> bool {
        self.pets.iter().any(|p| p.project.local.as_ref().is_some_and(|l| l.path == path))
    }

    /// Lists git repos directly inside each watched folder. Repos that appear in a folder already
    /// watched before become new-project candidates; a newly watched folder's repos don't (they're
    /// offered in "Adopt a pet" instead). Returns whether anything changed.
    pub fn scan_folders(&mut self) -> bool {
        let mut found = Vec::new();
        let mut fresh = Vec::new();
        for root in &self.settings.watched_folders {
            let seen_before = self.known_folders.roots.contains(root);
            let Ok(entries) = std::fs::read_dir(root) else { continue };
            for e in entries.flatten() {
                let path = e.path();
                if !path.join(".git").exists() {
                    continue;
                }
                let p = path.display().to_string();
                if self.known_folders.repos.insert(p.clone()) && seen_before && !self.tracked_folder(&p) {
                    fresh.push(p.clone());
                }
                found.push(p);
            }
            self.known_folders.roots.insert(root.clone());
        }
        found.sort();
        let changed = found != self.local_repos || !fresh.is_empty();
        self.local_repos = found;
        for f in fresh {
            if !self.new_folders.contains(&f) {
                self.new_folders.push(f);
            }
        }
        if changed && let Err(e) = self.store.save(FOLDERS_FILE, &self.known_folders) {
            eprintln!("could not save known folders: {e:?}");
        }
        changed
    }

    /// Reads commit dates from pets' local folders (§5.4). Never runs git or fetches.
    pub fn refresh_local_activity(&mut self) -> bool {
        let mut changed = Vec::new();
        for p in &mut self.pets {
            let Some(path) = p.project.local.as_ref().map(|l| l.path.clone()).filter(|p| !p.is_empty()) else { continue };
            if matches!(p.state.kind, StateKind::Graduated | StateKind::LetGo) {
                continue;
            }
            let Ok(info) = localgit::read(std::path::Path::new(&path), &Default::default()) else { continue };
            let before = p.activity.last;
            for (t, s) in [(info.last_commit, ActivitySource::LocalCommit), (info.last_remote_ref, ActivitySource::LocalRemoteRef)] {
                if let Some(t) = t {
                    p.record_activity(t, s);
                }
            }
            if p.activity.last != before {
                changed.push(p.id);
                self.cues.push((p.id, Cue::Eat));
            }
        }
        changed.iter().for_each(|&id| self.save_pet(id));
        !changed.is_empty()
    }

    /// At most one nudge from an on-screen pet (§5.10); the rules live in `strays_core::nudge`.
    pub fn next_nudge(&mut self, now: Time, full_screen: bool) -> Option<strays_core::nudge::Nudge> {
        use strays_core::chrono::Timelike;
        use strays_core::nudge::{Candidate, Context, pick};
        let local = now.with_timezone(&Local);
        let cx = Context {
            now,
            local_day: local.date_naive(),
            local_minute: local.hour() * 60 + local.minute(),
            presentation: self.flags.presentation,
            full_screen,
        };
        let on_screen: Vec<Candidate> = self
            .visible
            .iter()
            .filter_map(|id| self.pets.iter().find(|p| p.id == *id))
            .map(|p| Candidate { id: p.id, repo: p.project.github.as_ref().map(|g| g.full_name.as_str()), mood: self.mood(p, now) })
            .collect();
        let all: Vec<Mood> = self.pets.iter().filter(|p| p.state.kind == StateKind::Active).map(|p| self.mood(p, now)).collect();
        let mut log = self.nudge_log.clone();
        let nudge = pick(&on_screen, &all, &mut log, &self.settings, &cx);
        if log != self.nudge_log {
            self.nudge_log = log;
            if let Err(e) = self.store.save(NUDGE_FILE, &self.nudge_log) {
                eprintln!("could not save the nudge log: {e:?}");
            }
        }
        nudge
    }

    pub fn set_account(&mut self, account: Option<Account>) {
        self.account = account;
        if let Err(e) = self.store.save(ACCOUNT_FILE, &self.account) {
            eprintln!("could not save account: {e:?}");
        }
    }

    pub fn save_settings(&mut self, s: Settings) -> Result<(), String> {
        self.store.save_settings(&s).map_err(|e| format!("{e:?}"))?;
        self.settings = s;
        Ok(())
    }

    /// Appends to the network activity log, keeping the newest entries (§8.5).
    pub fn append_netlog(&self, entries: Vec<NetEntry>) {
        if entries.is_empty() {
            return;
        }
        let mut all = self.netlog();
        all.extend(entries);
        let start = all.len().saturating_sub(NETLOG_KEEP);
        let text: String = all[start..].iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect();
        if let Err(e) = std::fs::write(self.dir.join(NETLOG_FILE), text) {
            eprintln!("could not write the network log: {e}");
        }
    }

    pub fn netlog(&self) -> Vec<NetEntry> {
        std::fs::read_to_string(self.dir.join(NETLOG_FILE))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// The adoption flow (§8.2): a GitHub repo or a local folder (matched to its GitHub repo when the
    /// remote is unambiguous). Adopted if a slot is free, otherwise it waits in the shelter.
    pub fn adopt(&mut self, req: AdoptRequest, now: Time) -> Result<String, String> {
        let mut project = Project::default();
        let mut last: Option<Time> = None;
        let mut source = ActivitySource::Github;
        if let Some(id) = req.repo_id {
            if self.tracked(id) {
                return Err("This repo already has a pet".into());
            }
            let repo = self.repos.iter().chain(&self.new_projects).find(|r| r.id == id).cloned().ok_or("Unknown repo")?;
            project.github = Some(GithubRef { id: repo.id, full_name: repo.full_name.clone(), account: repo.owner.login.clone() });
            last = Some(repo.pushed_at.unwrap_or(repo.created_at));
        }
        if let Some(folder) = &req.folder {
            if self.pets.iter().any(|p| p.project.local.as_ref().is_some_and(|l| &l.path == folder)) {
                return Err("This folder already has a pet".into());
            }
            let info = localgit::read(std::path::Path::new(folder), &Default::default()).map_err(|e| format!("Not a git repository: {e}"))?;
            if project.github.is_none()
                && let localgit::Match::Unique(id) = localgit::match_repo(&info, &self.repos)
                && !self.tracked(id)
                && let Some(repo) = self.repos.iter().find(|r| r.id == id)
            {
                project.github = Some(GithubRef { id, full_name: repo.full_name.clone(), account: repo.owner.login.clone() });
                last = repo.pushed_at;
            }
            for (t, s) in [(info.last_commit, ActivitySource::LocalCommit), (info.last_remote_ref, ActivitySource::LocalRemoteRef)] {
                if let Some(t) = t
                    && last.is_none_or(|l| t > l)
                {
                    last = Some(t);
                    source = s;
                }
            }
            project.local = Some(LocalRef { bookmark_id: Uuid::new_v4(), display_path: tilde(folder), path: folder.clone() });
        }
        if project.github.is_none() && project.local.is_none() {
            return Err("Pick a repo or a folder".into());
        }
        let name = if req.name.trim().is_empty() { "Stray".to_string() } else { req.name.trim().chars().take(40).collect() };
        let mut p = Pet::new(now, &name, req.species, project, Activity { last: last.unwrap_or(now), source }, req.pace);
        p.colors = req.colors;
        p.goal = Goal { text: req.goal.trim().chars().take(200).collect(), checklist: Vec::new() };
        let message = if self.slot_free() {
            p.adopt(now).expect("new pets start in the shelter");
            self.cues.push((p.id, Cue::Hello));
            format!("{name} moved in")
        } else {
            format!("All slots are full, so {name} is waiting in the shelter")
        };
        if let Some(gh) = &p.project.github {
            let id = gh.id;
            self.new_projects.retain(|r| r.id != id);
        }
        let id = p.id;
        if let Some(f) = &req.folder {
            self.new_folders.retain(|n| n != f);
        }
        self.pets.push(p);
        self.save_pet(id);
        Ok(message)
    }

    pub fn set_goal(&mut self, id: Uuid, goal: Goal) -> Result<(), String> {
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        p.goal = Goal {
            text: goal.text.chars().take(200).collect(),
            checklist: goal.checklist.into_iter().take(30).map(|mut c| {
                c.text = c.text.chars().take(120).collect();
                c
            }).collect(),
        };
        self.save_pet(id);
        Ok(())
    }

    /// Pet settings: name and pace.
    pub fn set_pet(&mut self, id: Uuid, name: String, pace: Pace) -> Result<(), String> {
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        if !name.trim().is_empty() {
            p.name = name.trim().chars().take(40).collect();
        }
        p.pace = pace;
        self.save_pet(id);
        Ok(())
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

/// `/Users/me/Code/x` → `~/Code/x` for display.
pub fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if path.starts_with(&home) => format!("~{}", &path[home.len()..]),
        _ => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_repos_appearing_after_a_folder_is_watched_count_as_new() {
        let root = std::env::temp_dir().join(format!("strays-scan-{}", Uuid::new_v4()));
        let (data, code) = (root.join("data"), root.join("code"));
        std::fs::create_dir_all(code.join("old/.git")).unwrap();
        std::fs::create_dir_all(code.join("not-a-repo")).unwrap();
        let mut app = App::load(data.clone()).unwrap();
        app.settings.watched_folders = vec![code.display().to_string()];

        assert!(app.scan_folders());
        assert_eq!(app.local_repos, vec![code.join("old").display().to_string()]);
        assert!(app.new_folders.is_empty(), "repos already there when watching starts aren't new");

        std::fs::create_dir_all(code.join("fresh/.git")).unwrap();
        assert!(app.scan_folders());
        assert_eq!(app.new_folders, vec![code.join("fresh").display().to_string()]);
        assert!(!app.scan_folders(), "nothing changed");

        // the known list survives a restart, so "fresh" isn't offered twice
        let mut again = App::load(data).unwrap();
        again.settings.watched_folders = app.settings.watched_folders.clone();
        again.scan_folders();
        assert!(again.new_folders.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_graduate_leaves_wearing_its_accessory() {
        let now = strays_core::chrono::Utc::now();
        let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: None, local: None }, Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
        assert_eq!(Cue::graduate(&p), Cue::Graduate { accessory: 0 });
        p.graduate(now, "").unwrap();
        let Cue::Graduate { accessory } = Cue::graduate(&p) else { unreachable!() };
        assert_eq!(pet::ACCESSORIES[accessory as usize - 1], p.accessory.as_deref().unwrap());
        assert_eq!(serde_json::to_value(Cue::Graduate { accessory: 3 }).unwrap(), serde_json::json!({ "graduate": { "accessory": 3 } }));
    }
}
