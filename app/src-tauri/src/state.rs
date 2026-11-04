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
const BOOKMARKS_FILE: &str = "bookmarks.json";
const PROMPTS_FILE: &str = "new-projects.json";
/// Every repo the token sees, from the last sync, for "Adopt a project" and folder matching before the next one.
const REPOS_FILE: &str = "repos.json";

/// New repos and folders still waiting for the user's decision (§5.9). Saved, because once seen they
/// count as known and would never be offered again.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Prompts {
    repos: Vec<Repo>,
    folders: Vec<String>,
}

/// Git folders already seen inside watched folders, so only ones that appear later count as new (§5.9).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct KnownFolders {
    roots: std::collections::BTreeSet<String>,
    repos: std::collections::BTreeSet<String>,
}

/// The connected GitHub account (no secrets; the token itself is in the Keychain).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub login: String,
    pub expires: Option<Time>,
    /// Older `accounts.json` files wrote `repo_count`.
    #[serde(alias = "repo_count")]
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
    /// Whether the Keychain holds a token, as last seen by the sync thread or the setup flow; the UI reads this, not the Keychain.
    pub has_token: bool,
    /// Git repos found directly inside the watched folders.
    pub local_repos: Vec<String>,
    /// Ones that appeared after their folder was first watched, waiting for the user's decision.
    pub new_folders: Vec<String>,
    known_folders: KnownFolders,
    nudge_log: strays_core::nudge::NudgeLog,
    /// Security-scoped bookmarks for folders the user picked, by path (§9): the sandbox's way back in after a restart.
    bookmarks: std::collections::BTreeMap<String, Vec<u8>>,
    /// Problems the user should know about, shown in the menu and Settings until fixed or dismissed.
    pub issues: Vec<Issue>,
    /// Error reports waiting for the sync thread to send them (only collected while the user opted in).
    pub reports: Vec<crate::report::Report>,
    /// Clips waiting to be played; `Eat` waits until its pet is on screen.
    pub cues: Vec<(Uuid, Cue)>,
}

/// Something that went wrong. `key` identifies the cause, so a repeat replaces it and a fix can clear it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Issue {
    pub key: String,
    pub text: String,
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
    /// GitHub can't find its repo; the overlay asks the user what happened.
    pub confused: bool,
    /// Its own liveliness, if it doesn't follow the app's.
    pub liveliness: Option<strays_core::settings::Liveliness>,
    /// Where the user dragged it, if anywhere.
    #[serde(skip)]
    pub home: Option<strays_core::pet::Home>,
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
        let store = Store::open(dir.clone()).map_err(|e| e.to_string())?;
        let mut issues = Vec::new();
        let settings = store.load_settings().unwrap_or_else(|e| reset(&store, "settings.json", "Your settings", e, &mut issues));
        let (mut pets, problems) = store.load_pets().map_err(|e| e.to_string())?;
        // a future-dated commit read by an older version would pin its pet at Joyful until that date
        let now = strays_core::chrono::Utc::now();
        for p in pets.iter_mut().filter(|p| p.activity.last > now) {
            p.activity.last = now;
            if let Err(e) = store.save_pet(p) {
                issues.push(Issue { key: format!("save:{}", p.id), text: format!("Couldn't save {}: {e}", p.name) });
            }
        }
        for p in &problems {
            issues.push(Issue { key: format!("load:{p}"), text: p.to_string() });
        }
        let sync: SyncState = load(&store, SYNC_FILE, "Sync history", &mut issues);
        let account: Option<Account> = load(&store, ACCOUNT_FILE, "The GitHub account details", &mut issues);
        let known_folders: KnownFolders = load(&store, FOLDERS_FILE, "The list of known project folders", &mut issues);
        let nudge_log = load(&store, NUDGE_FILE, "The nudge history", &mut issues);
        let mut bookmarks: std::collections::BTreeMap<String, Vec<u8>> = load(&store, BOOKMARKS_FILE, "Folder access", &mut issues);
        let prompts: Prompts = load(&store, PROMPTS_FILE, "The list of new projects", &mut issues);
        let repos: Vec<Repo> = load(&store, REPOS_FILE, "The list of GitHub repos", &mut issues);
        // a bookmark no watched folder or pet still needs is dropped, so the sandbox stops granting that folder
        let needed = needed_folders(&settings, &pets, strays_core::chrono::Utc::now());
        let before = bookmarks.len();
        bookmarks.retain(|b, _| needed.iter().any(|n| std::path::Path::new(n).starts_with(b)));
        if bookmarks.len() != before
            && let Err(e) = store.save(BOOKMARKS_FILE, &bookmarks)
        {
            issues.push(Issue { key: "save:bookmarks".into(), text: format!("Couldn't save folder access: {e}") });
        }
        // vacation lives in the pets it paused, so it survives a restart without a file of its own
        let vacation = pets.iter().any(|p| p.state.kind == StateKind::Paused && p.state.reason == "vacation");
        let mut app = Self {
            store,
            dir,
            account,
            settings,
            pets,
            sync,
            flags: Flags { vacation, ..Flags::default() },
            repos,
            new_projects: prompts.repos,
            visible: Vec::new(),
            hidden_needy: 0,
            last_shown: HashMap::new(),
            rotation_started: None,
            sync_error: None,
            has_token: false,
            issues,
            reports: Vec::new(),
            cues: Vec::new(),
            local_repos: Vec::new(),
            new_folders: prompts.folders,
            known_folders,
            nudge_log,
            bookmarks,
        };
        // problems found while loading go through the same log and reports as later ones
        for i in std::mem::take(&mut app.issues) {
            app.warn(&i.key, i.text);
        }
        app.note_missing();
        app.pick_up_crash();
        Ok(app)
    }

    /// Records a problem for the user (menu and Settings) and the log. A repeat of the same `key` replaces it.
    pub fn warn(&mut self, key: &str, text: impl Into<String>) {
        let text = text.into();
        match self.issues.iter_mut().find(|i| i.key == key) {
            Some(i) if i.text == text => return,
            Some(i) => i.text = text.clone(),
            None => self.issues.push(Issue { key: key.into(), text: text.clone() }),
        }
        // logged when it starts or changes: a problem that persists mustn't push older lines out of the log
        crate::log::warn(&format!("{key}: {text}"));
        // reported once when it starts or changes, not on every repeat
        self.queue_report(key, &text);
    }

    /// Names that must never leave the Mac in a report or a sent log: pets, repos, the account, and
    /// every part of the folders the user picked (a folder name like `ClientX-NDA` can be private too).
    pub(crate) fn private_words(&self) -> Vec<String> {
        let mut v: Vec<String> = self.account.iter().map(|a| a.login.clone()).collect();
        let mut folders: Vec<&str> = self.settings.watched_folders.iter().map(String::as_str).collect();
        for p in &self.pets {
            v.push(p.name.clone());
            if let Some(g) = &p.project.github {
                v.push(g.full_name.clone());
                v.extend(g.full_name.split('/').map(String::from));
            }
            if let Some(l) = &p.project.local {
                folders.extend([l.display_path.as_str(), l.path.as_str()]);
            }
        }
        v.extend(self.repos.iter().map(|r| r.full_name.clone()));
        v.extend(folders.iter().flat_map(|f| f.split('/')).filter(|part| part.len() >= 3 && *part != "Users").map(String::from));
        // the Mac's user name, wherever a path to the home folder ends up
        if let Some(user) = std::env::var("HOME").ok().and_then(|h| h.split("/Library/Containers/").next().and_then(|h| h.rsplit('/').next()).map(String::from)) {
            v.push(user);
        }
        v
    }

    fn queue_report(&mut self, key: &str, text: &str) {
        if !self.settings.error_reports || !crate::report::available() {
            return;
        }
        let private = self.private_words();
        let private: Vec<&str> = private.iter().map(String::as_str).collect();
        if let Some(r) = crate::report::for_issue(key, text, &private) {
            self.reports.push(r);
            let extra = self.reports.len().saturating_sub(crate::report::QUEUE);
            self.reports.drain(..extra);
        }
    }

    /// The last run crashed: tell the user, report it if they opted in, and keep the details in the log.
    fn pick_up_crash(&mut self) {
        let path = self.dir.join(crate::report::CRASH_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        self.issues.push(Issue { key: "crash".into(), text: "Strays quit unexpectedly last time. The details are in logs/strays.log in the data folder (Settings → Privacy).".into() });
        if self.settings.error_reports && crate::report::available() {
            let private = self.private_words();
            let private: Vec<&str> = private.iter().map(String::as_str).collect();
            self.reports.push(crate::report::for_crash(&text, &private));
        }
        if let Err(e) = std::fs::remove_file(&path) {
            self.warn("crash-file", format!("Couldn't clear the crash note, so this warning may show again: {e}"));
        }
    }

    /// Clears a problem once its cause is fixed.
    pub fn resolve(&mut self, key: &str) {
        let before = self.issues.len();
        self.issues.retain(|i| i.key != key);
        if self.issues.len() != before {
            crate::log::write("INFO", &format!("{key}: resolved"));
        }
    }

    /// Records a save's outcome under `key`: a failure warns, a success clears an earlier failure.
    fn saved(&mut self, key: &str, what: &str, r: Result<(), strays_core::store::StoreError>) {
        match r {
            Ok(()) => self.resolve(key),
            Err(e) => self.warn(key, format!("Couldn't save {what}: {e}")),
        }
    }

    pub fn remember_bookmark(&mut self, path: String, data: Vec<u8>) {
        self.bookmarks.insert(path, data);
        self.save_bookmarks();
    }

    /// Opens every saved bookmark (`open` resolves one and starts access), replacing stale ones.
    /// ponytail: a folder that moved keeps working only under its old path; pets and watched folders aren't renamed.
    pub fn open_bookmarks(&mut self, open: impl Fn(&[u8]) -> Result<(String, Option<Vec<u8>>), String>) {
        let mut changed = false;
        let mut lost = Vec::new();
        for (path, data) in self.bookmarks.iter_mut() {
            match open(data) {
                Ok((now_at, fresh)) => {
                    if &now_at != path {
                        crate::log::write("INFO", &format!("folder {path} moved to {now_at}"));
                    }
                    if let Some(fresh) = fresh {
                        *data = fresh;
                        changed = true;
                    }
                }
                Err(e) => lost.push((path.clone(), e)),
            }
        }
        for (path, e) in lost {
            self.warn(&format!("access:{path}"), format!("Strays can't open {} any more ({e}). Add the folder again in Settings → Folders.", tilde(&path)));
        }
        if changed {
            self.save_bookmarks();
        }
    }

    fn save_bookmarks(&mut self) {
        let r = self.store.save(BOOKMARKS_FILE, &self.bookmarks);
        self.saved("save:bookmarks", "folder access", r);
    }

    pub fn save_pet(&mut self, id: Uuid) {
        let Some(p) = self.pets.iter().find(|p| p.id == id) else { return };
        let (r, name) = (self.store.save_pet(p), p.name.clone());
        self.saved(&format!("save:{id}"), &name, r);
    }

    pub fn save_all(&mut self) {
        let ids: Vec<Uuid> = self.pets.iter().map(|p| p.id).collect();
        ids.into_iter().for_each(|id| self.save_pet(id));
        let r = self.store.save(SYNC_FILE, &self.sync);
        self.saved("save:sync", "the sync history", r);
    }

    pub fn mood(&self, p: &Pet, now: Time) -> Mood {
        p.mood(now, self.sync.last_successful_check, &self.settings, &Local)
    }

    /// A pet keeps its project until it graduates or its let-go can no longer be undone; then the project can be adopted again.
    fn holds_project(&self, p: &Pet) -> bool {
        match p.state.kind {
            StateKind::Graduated => false,
            StateKind::LetGo => strays_core::chrono::Utc::now() - p.state.since <= Duration::days(self.settings.let_go_undo_days.into()),
            _ => true,
        }
    }

    pub fn tracked(&self, repo_id: u64) -> bool {
        self.pets.iter().any(|p| self.holds_project(p) && p.project.github.as_ref().is_some_and(|g| g.id == repo_id))
    }

    pub fn slot_free(&self) -> bool {
        pet::slots_used(&self.pets) < self.settings.slots as usize
    }

    /// Ends due snoozes, holidays and scheduled resumes. Returns whether anything changed.
    pub fn tick(&mut self, now: Time) -> bool {
        let mut changed = Vec::new();
        // pets whose snoozes end together come back `wake_stagger` minutes apart (§5.6)
        let gap = Duration::minutes(self.settings.wake_stagger.into());
        let mut due: Vec<&mut Pet> = self.pets.iter_mut().filter(|p| p.state.kind == StateKind::Snoozed && p.state.until.is_some_and(|u| u <= now)).collect();
        due.sort_by_key(|p| (p.state.until, p.created_at));
        for (i, p) in due.into_iter().enumerate().skip(1).filter(|_| gap > Duration::zero()) {
            p.state.until = Some(now + gap * i as i32);
            changed.push(p.id);
        }
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
        let sel = display::choose(&self.pets, &moods, &self.last_shown, self.settings.min_visible as usize, self.settings.max_visible as usize);
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
                PetView { id: p.id, species: p.species, colors: p.colors.clone(), label, mood, confused: p.missing_since.is_some(), liveliness: p.liveliness, home: p.home }
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
        self.save_prompts();
        self.save_pet(id);
        Ok(message)
    }

    pub fn ignore_folder(&mut self, path: &str) {
        self.new_folders.retain(|f| f != path);
        self.save_prompts();
    }

    pub fn ignore_new(&mut self, repo_id: u64) {
        self.new_projects.retain(|r| r.id != repo_id);
        self.save_prompts();
    }

    /// Keeps `repos` as the latest view of every repo, newest push first. A `complete` listing replaces it,
    /// so deleted repos and ones the token no longer sees drop out; a partial one only updates.
    pub fn merge_repos(&mut self, repos: &[Repo], complete: bool) {
        if complete {
            self.repos = repos.to_vec();
        } else {
            for r in repos {
                match self.repos.iter_mut().find(|x| x.id == r.id) {
                    Some(x) => *x = r.clone(),
                    None => self.repos.push(r.clone()),
                }
            }
        }
        self.repos.sort_by_key(|r| std::cmp::Reverse(r.pushed_at));
        let r = self.store.save(REPOS_FILE, &self.repos);
        self.saved("save:repos", "the list of GitHub repos", r);
    }

    /// Repos found by a sync that wait for the user's decision.
    pub fn offer_new(&mut self, repos: &[Repo]) {
        let fresh: Vec<Repo> = repos.iter().filter(|r| !self.new_projects.iter().any(|n| n.id == r.id)).cloned().collect();
        if !fresh.is_empty() {
            self.new_projects.extend(fresh);
            self.save_prompts();
        }
    }

    fn save_prompts(&mut self) {
        let prompts = Prompts { repos: self.new_projects.clone(), folders: self.new_folders.clone() };
        let r = self.store.save(PROMPTS_FILE, &prompts);
        self.saved("save:prompts", "the list of new projects", r);
    }

    pub fn act(&mut self, id: Uuid, action: Action, now: Time) -> Result<String, String> {
        let slot_free = self.slot_free();
        let settings = self.settings.clone();
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        let name = p.name.clone();
        let refused = |e: pet::Refused| e.to_string();
        let msg = match action {
            Action::Adopt if !slot_free => return Err("All adoption slots are full".into()),
            Action::Adopt => {
                p.adopt(now).map_err(refused)?;
                self.cues.push((id, Cue::Hello));
                format!("{name} moved in. Hold {} and click {name} to see its card.", self.settings.modifier.symbol())
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
        self.note_missing();
        Ok(msg)
    }

    /// One warning naming every pet whose repo GitHub can't find, so it's seen even with the pet off screen.
    pub fn note_missing(&mut self) {
        let names: Vec<String> = self.pets.iter().filter(|p| p.missing_since.is_some() && self.holds_project(p) && p.state.kind != StateKind::LetGo).map(|p| p.name.clone()).collect();
        if names.is_empty() {
            return self.resolve("missing");
        }
        self.warn(
            "missing",
            format!(
                "GitHub can't find the repo of {}. It may have been deleted, or your token can't see it any more. ⌥-click the pet, or use its menu, to decide.",
                names.join(", ")
            ),
        );
    }

    pub fn tracked_folder(&self, path: &str) -> bool {
        self.pets.iter().any(|p| self.holds_project(p) && p.project.local.as_ref().is_some_and(|l| l.path == path))
    }

    /// Lists git repos directly inside each watched folder. Repos that appear in a folder already
    /// watched before become new-project candidates; a newly watched folder's repos don't (they're
    /// offered in "Adopt a pet" instead). Returns whether anything changed.
    #[cfg(test)]
    pub fn scan_folders(&mut self) -> bool {
        let listed = list_git_folders(&self.settings.watched_folders);
        self.apply_scan(listed)
    }

    /// Records what `list_git_folders` found (read without holding the state). Returns whether anything changed.
    pub fn apply_scan(&mut self, listed: Vec<(String, Result<Vec<String>, String>)>) -> bool {
        let mut found = Vec::new();
        let mut fresh = Vec::new();
        let mut unreadable = Vec::new();
        for (root, repos) in listed {
            // the folder may have been removed from the watch list while it was being read
            if !self.settings.watched_folders.contains(&root) {
                continue;
            }
            let repos = match repos {
                Ok(r) => {
                    self.resolve(&format!("scan:{root}"));
                    r
                }
                Err(e) => {
                    unreadable.push((root, e));
                    continue;
                }
            };
            let seen_before = self.known_folders.roots.contains(&root);
            for p in repos {
                if self.known_folders.repos.insert(p.clone()) && seen_before && !self.tracked_folder(&p) {
                    fresh.push(p.clone());
                }
                found.push(p);
            }
            self.known_folders.roots.insert(root);
        }
        for (root, e) in unreadable {
            self.warn(&format!("scan:{root}"), format!("Strays can't look inside {} ({e}). Add it again in Settings → Folders, or remove it.", tilde(&root)));
        }
        found.sort();
        let changed = found != self.local_repos || !fresh.is_empty();
        self.local_repos = found;
        let offered = self.new_folders.len();
        for f in fresh {
            if !self.new_folders.contains(&f) {
                self.new_folders.push(f);
            }
        }
        if self.new_folders.len() != offered {
            self.save_prompts();
        }
        if changed {
            let r = self.store.save(FOLDERS_FILE, &self.known_folders);
            self.saved("save:folders", "the list of known folders", r);
        }
        changed
    }

    /// Local folders to read for activity (§5.4): pets that still follow theirs.
    pub fn local_jobs(&self) -> Vec<(Uuid, String)> {
        self.pets
            .iter()
            .filter(|p| !matches!(p.state.kind, StateKind::Graduated | StateKind::LetGo))
            .filter_map(|p| p.project.local.as_ref().filter(|l| !l.path.is_empty()).map(|l| (p.id, l.path.clone())))
            .collect()
    }

    /// Records what `read_local` found (read without holding the state). Returns whether a pet got new activity.
    pub fn apply_local(&mut self, results: Vec<(Uuid, Result<localgit::LocalInfo, String>)>) -> bool {
        let now = strays_core::chrono::Utc::now();
        let mut changed = Vec::new();
        for (id, info) in results {
            let Some(p) = self.pets.iter_mut().find(|p| p.id == id && !matches!(p.state.kind, StateKind::Graduated | StateKind::LetGo)) else { continue };
            let info = match info {
                Ok(info) => info.no_later_than(now),
                Err(e) => {
                    let (name, folder) = (p.name.clone(), p.project.local.as_ref().map(|l| l.display_path.clone()).unwrap_or_default());
                    self.warn(&format!("local:{id}"), format!("Strays can't read {folder} for {name} ({e}). If the folder moved or was deleted, let {name} go, or adopt the folder again."));
                    continue;
                }
            };
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
            self.resolve(&format!("local:{id}"));
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
            let r = self.store.save(NUDGE_FILE, &self.nudge_log);
            self.saved("save:nudges", "the nudge history", r);
        }
        nudge
    }

    pub fn set_account(&mut self, account: Option<Account>) {
        self.account = account;
        let r = self.store.save(ACCOUNT_FILE, &self.account);
        self.saved("save:account", "the GitHub account details", r);
    }

    pub fn save_settings(&mut self, s: Settings) -> Result<(), String> {
        self.store.save_settings(&s).map_err(|e| e.to_string())?;
        self.settings = s;
        Ok(())
    }

    /// Appends to the network activity log, keeping the newest entries (§8.5).
    /// Delete everything: nothing may write the data folder from here on (`Store::close`).
    pub fn close(&mut self) {
        self.store.close();
    }

    pub fn is_closed(&self) -> bool {
        self.store.is_closed()
    }

    pub fn append_netlog(&mut self, entries: Vec<NetEntry>) {
        if entries.is_empty() || self.is_closed() {
            return;
        }
        let path = self.dir.join(NETLOG_FILE);
        let written = match self.read_netlog() {
            Ok(mut all) => {
                all.extend(entries);
                let start = all.len().saturating_sub(NETLOG_KEEP);
                std::fs::write(&path, lines(&all[start..]))
            }
            // the file is there but couldn't be read: add to it rather than replace what it holds
            Err(e) => {
                crate::log::warn(&format!("could not read the network log, so new entries are added to it untrimmed: {e}"));
                use std::io::Write;
                std::fs::OpenOptions::new().append(true).open(&path).and_then(|mut f| f.write_all(lines(&entries).as_bytes()))
            }
        };
        match written {
            Ok(()) => self.resolve("save:netlog"),
            Err(e) => self.warn("save:netlog", format!("Couldn't write the network log: {e}")),
        }
    }

    /// The network activity log. Unreadable lines are skipped (and logged): one bad line mustn't hide the rest.
    pub fn netlog(&self) -> Vec<NetEntry> {
        self.read_netlog().unwrap_or_else(|e| {
            crate::log::warn(&format!("could not read the network log: {e}"));
            Vec::new()
        })
    }

    fn read_netlog(&self) -> std::io::Result<Vec<NetEntry>> {
        let text = match std::fs::read_to_string(self.dir.join(NETLOG_FILE)) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut bad = 0;
        let entries = text.lines().filter_map(|l| serde_json::from_str(l).inspect_err(|_| bad += 1).ok()).collect();
        if bad > 0 {
            crate::log::warn(&format!("{bad} unreadable line(s) in the network log were skipped"));
        }
        Ok(entries)
    }

    /// The adoption flow (§8.2): a GitHub repo or a local folder (matched to its GitHub repo when the
    /// remote is unambiguous). Adopted if a slot is free, otherwise it waits in the shelter.
    pub fn adopt(&mut self, req: AdoptRequest, now: Time) -> Result<String, String> {
        // colours come from the webview and are saved in the pet file: Rive property names and #RRGGBB only
        let hex = |v: &str| v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit());
        let name_ok = |k: &str| (1..=32).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_alphanumeric());
        if req.colors.len() > 16 || !req.colors.iter().all(|(k, v)| name_ok(k) && hex(v)) {
            return Err("Those colours aren't valid".into());
        }
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
            if self.tracked_folder(folder) {
                return Err("This folder already has a pet".into());
            }
            let info = localgit::read(std::path::Path::new(folder), &Default::default()).map_err(|e| format!("Not a git repository: {e}"))?.no_later_than(now);
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
            format!("{name} moved in. Hold {} and click {name} to see its card.", self.settings.modifier.symbol())
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
        self.save_prompts();
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
    pub fn set_pet(&mut self, id: Uuid, name: String, pace: Pace, liveliness: Option<strays_core::settings::Liveliness>) -> Result<(), String> {
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        if !name.trim().is_empty() {
            p.name = name.trim().chars().take(40).collect();
        }
        p.pace = pace;
        p.liveliness = liveliness;
        self.save_pet(id);
        Ok(())
    }

    /// Moves a pet to a home spot, or (None) back into the automatic line-up.
    pub fn set_home(&mut self, id: Uuid, home: Option<strays_core::pet::Home>) -> Result<(), String> {
        let p = self.pets.iter_mut().find(|p| p.id == id).ok_or("No such pet")?;
        p.home = home;
        self.save_pet(id);
        Ok(())
    }

    /// Puts every dragged pet back into the automatic line-up.
    pub fn clear_homes(&mut self) {
        let moved: Vec<Uuid> = self.pets.iter_mut().filter_map(|p| p.home.take().map(|_| p.id)).collect();
        for id in moved {
            self.save_pet(id);
        }
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

/// Network log entries as JSON lines.
fn lines(entries: &[NetEntry]) -> String {
    entries.iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect()
}

fn load<T: serde::de::DeserializeOwned + Default>(store: &Store, name: &str, what: &str, issues: &mut Vec<Issue>) -> T {
    store.load(name).unwrap_or_else(|e| reset(store, name, what, e, issues))
}

/// A data file that couldn't be loaded: a corrupt one is moved aside (one copy, replacing any older copy) so
/// the fresh file written next can't destroy it, and the user is told. Returns the defaults to start from.
fn reset<T: Default>(store: &Store, name: &str, what: &str, e: strays_core::store::StoreError, issues: &mut Vec<Issue>) -> T {
    use strays_core::store::StoreError;
    let text = match e {
        StoreError::Corrupt { .. } => match store.set_aside(name) {
            Ok(kept) => format!("{what} couldn't be read, so Strays started fresh. The old file was kept as {}.", kept.display()),
            Err(e2) => format!("{what} couldn't be read and couldn't be moved aside ({e2}), so the next save will replace it."),
        },
        e => format!("{what} couldn't be read ({e}). Strays is using defaults for now."),
    };
    issues.push(Issue { key: format!("load:{name}"), text });
    T::default()
}

/// Git repos directly inside each watched folder (§5.9). Slow-ish on big folders, so it runs without the state.
pub fn list_git_folders(roots: &[String]) -> Vec<(String, Result<Vec<String>, String>)> {
    roots
        .iter()
        .map(|root| {
            let listed = std::fs::read_dir(root).map_err(|e| e.to_string()).map(|entries| {
                entries.flatten().map(|e| e.path()).filter(|p| p.join(".git").exists()).map(|p| p.display().to_string()).collect()
            });
            (root.clone(), listed)
        })
        .collect()
}

/// Git change fingerprints from the last read, by pet: a folder whose fingerprint hasn't moved isn't read again.
pub type Fingerprints = HashMap<Uuid, Vec<(String, std::time::SystemTime)>>;

/// Reads the folders `jobs` names (§5.4), skipping ones unchanged since the last read. Never runs git or
/// fetches, and runs without the state, as a cold read takes up to ~150 ms a repo.
pub fn read_local(jobs: Vec<(Uuid, String)>, seen: &mut Fingerprints) -> Vec<(Uuid, Result<localgit::LocalInfo, String>)> {
    seen.retain(|id, _| jobs.iter().any(|(j, _)| j == id));
    let mut out = Vec::new();
    for (id, path) in jobs {
        let path = std::path::Path::new(&path);
        let fp = localgit::fingerprint(path);
        if fp.is_some() && seen.get(&id) == fp.as_ref() {
            continue;
        }
        let info = localgit::read(path, &Default::default());
        // only a good read is remembered, so a failing folder is tried again next time
        if let (Ok(_), Some(fp)) = (&info, fp) {
            seen.insert(id, fp);
        } else {
            seen.remove(&id);
        }
        out.push((id, info));
    }
    out
}

/// Folders the app still reads: watched folders, and those of pets that aren't finished or past their let-go undo.
fn needed_folders<'a>(settings: &'a Settings, pets: &'a [Pet], now: Time) -> Vec<&'a str> {
    let undo = Duration::days(settings.let_go_undo_days.into());
    let alive = |p: &&Pet| match p.state.kind {
        StateKind::Graduated => false,
        StateKind::LetGo => now - p.state.since <= undo,
        _ => true,
    };
    let pet_folders = pets.iter().filter(alive).filter_map(|p| p.project.local.as_ref().map(|l| l.path.as_str()));
    settings.watched_folders.iter().map(String::as_str).chain(pet_folders).collect()
}

/// "Until tomorrow" means 09:00 local the next day (§5.6).
pub fn snooze_until(s: Snooze, now: Time) -> Option<Time> {
    match s {
        Snooze::Hours(h) => Some(now + Duration::hours(h)),
        Snooze::Days(d) => Some(now + Duration::days(d)),
        Snooze::UntilWake => None,
        Snooze::Tomorrow => {
            use strays_core::chrono::Timelike;
            // just after midnight, "tomorrow" still means the coming morning, not the one after
            let local = now.with_timezone(&Local);
            let day = local.date_naive() + Duration::days(if local.hour() < 5 { 0 } else { 1 });
            let nine = day.and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
            Local.from_local_datetime(&nine).earliest().map(|t| t.to_utc()).or(Some(now + Duration::hours(24)))
        }
    }
}

/// `/Users/me/Code/x` → `~/Code/x` for display.
pub fn tilde(path: &str) -> String {
    let Ok(home) = std::env::var("HOME") else { return path.to_string() };
    // in the App Sandbox HOME is the app's container (~/Library/Containers/<id>/Data)
    let home = home.split("/Library/Containers/").next().unwrap_or(&home);
    match path.strip_prefix(home) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
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

        // both lists survive a restart: "fresh" is still offered, and only once
        let mut again = App::load(data.clone()).unwrap();
        again.settings.watched_folders = app.settings.watched_folders.clone();
        again.scan_folders();
        assert_eq!(again.new_folders, vec![code.join("fresh").display().to_string()]);
        // once decided, it's gone for good
        again.ignore_folder(&code.join("fresh").display().to_string());
        let mut third = App::load(data).unwrap();
        third.settings.watched_folders = app.settings.watched_folders.clone();
        third.scan_folders();
        assert!(third.new_folders.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_new_repo_offered_before_a_restart_is_still_offered_after_it() {
        let dir = std::env::temp_dir().join(format!("strays-prompts-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let repo = |id| Repo { id, name: "x".into(), full_name: format!("me/x{id}"), owner: strays_core::github::Owner { login: "me".into() }, private: true, fork: false, archived: false, created_at: now, pushed_at: Some(now) };
        let mut app = App::load(dir.clone()).unwrap();
        app.offer_new(&[repo(1), repo(2)]);
        app.offer_new(&[repo(1)]);
        let mut app = App::load(dir.clone()).unwrap();
        assert_eq!(app.new_projects.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2], "kept, once each");
        app.ignore_new(1);
        app.adopt_repo(2, Some(Species::Dog), now).unwrap();
        assert!(App::load(dir.clone()).unwrap().new_projects.is_empty(), "decided ones stay decided");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_repo_list_is_there_before_the_first_sync_and_a_full_listing_replaces_it() {
        let dir = std::env::temp_dir().join(format!("strays-repos-{}", Uuid::new_v4()));
        let t = |d: i64| strays_core::chrono::Utc::now() - Duration::days(d);
        let repo = |id, pushed| Repo { id, name: "x".into(), full_name: format!("me/x{id}"), owner: strays_core::github::Owner { login: "me".into() }, private: true, fork: false, archived: false, created_at: t(9), pushed_at: Some(pushed) };
        let mut app = App::load(dir.clone()).unwrap();
        app.merge_repos(&[repo(1, t(3)), repo(2, t(2))], true);
        // a partial listing updates what it names and keeps the rest
        app.merge_repos(&[repo(1, t(1))], false);
        let app = App::load(dir.clone()).unwrap();
        assert_eq!(app.repos.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2], "kept across a restart, newest push first");
        let mut app = app;
        app.merge_repos(&[repo(2, t(2))], true);
        assert_eq!(App::load(dir.clone()).unwrap().repos.iter().map(|r| r.id).collect::<Vec<_>>(), [2], "gone from a full listing, gone here");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pet_pinned_by_a_future_commit_is_freed_on_load() {
        let dir = std::env::temp_dir().join(format!("strays-future-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let mut app = App::load(dir.clone()).unwrap();
        let p = Pet::new(now, "Miso", Species::Cat, Project::default(), Activity { last: now + Duration::days(300), source: ActivitySource::LocalCommit }, Pace::Normal);
        let id = p.id;
        app.pets.push(p);
        app.save_pet(id);
        let app = App::load(dir.clone()).unwrap();
        assert!(app.pets[0].activity.last <= strays_core::chrono::Utc::now());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn until_tomorrow_after_midnight_means_this_morning() {
        let at = |h, m| Local.from_local_datetime(&Local::now().date_naive().and_hms_opt(h, m, 0).unwrap()).earliest().unwrap().to_utc();
        let nine_today = at(9, 0);
        assert_eq!(snooze_until(Snooze::Tomorrow, at(0, 30)), Some(nine_today));
        assert_eq!(snooze_until(Snooze::Tomorrow, at(4, 59)), Some(nine_today));
        let evening = snooze_until(Snooze::Tomorrow, at(22, 0)).unwrap();
        assert!(evening > nine_today && evening - nine_today <= Duration::hours(25), "the next morning");
    }

    #[test]
    fn one_bad_line_doesnt_hide_the_rest_of_the_network_log() {
        let dir = std::env::temp_dir().join(format!("strays-netlog-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        let entry = NetEntry { at: strays_core::chrono::Utc::now(), method: "GET".into(), host: "api.github.com".into(), path: "/".into(), status: 200, reason: "scheduled".into() };
        app.append_netlog(vec![entry.clone()]);
        let mut text = std::fs::read_to_string(dir.join(NETLOG_FILE)).unwrap();
        text.push_str("{half a line\n");
        std::fs::write(dir.join(NETLOG_FILE), text).unwrap();
        assert_eq!(app.netlog(), vec![entry.clone()]);
        // the next write keeps only readable lines
        app.append_netlog(vec![entry.clone()]);
        assert!(!std::fs::read_to_string(dir.join(NETLOG_FILE)).unwrap().contains("half"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unreadable_network_log_is_added_to_not_replaced() {
        let dir = std::env::temp_dir().join(format!("strays-netlog-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        let entry = NetEntry { at: strays_core::chrono::Utc::now(), method: "GET".into(), host: "api.github.com".into(), path: "/".into(), status: 200, reason: "scheduled".into() };
        // not UTF-8, so it can't be read as text
        std::fs::write(dir.join(NETLOG_FILE), b"\xff history\n").unwrap();
        app.append_netlog(vec![entry.clone()]);
        let bytes = std::fs::read(dir.join(NETLOG_FILE)).unwrap();
        assert!(bytes.starts_with(b"\xff history\n"), "the old contents are kept");
        assert!(String::from_utf8_lossy(&bytes).contains("api.github.com"), "and the new entry is added");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tilde_uses_the_real_home_inside_the_sandbox() {
        let home = std::env::var("HOME").unwrap();
        let real = home.split("/Library/Containers/").next().unwrap().to_string();
        assert_eq!(tilde(&format!("{real}/Code/x")), "~/Code/x");
        assert_eq!(tilde(&format!("{real}x/Code")), format!("{real}x/Code"), "a sibling folder isn't home");
        assert_eq!(tilde("/opt/x"), "/opt/x");
    }

    #[test]
    fn folder_bookmarks_survive_a_restart_and_stale_ones_are_replaced() {
        let root = std::env::temp_dir().join(format!("strays-bm-{}", Uuid::new_v4()));
        let code = root.join("code");
        std::fs::create_dir_all(&code).unwrap();
        let path = code.display().to_string();
        let mut app = App::load(root.join("data")).unwrap();
        app.save_settings(Settings { watched_folders: vec![path.clone()], ..Settings::default() }).unwrap();
        let data = crate::platform::bookmark(&path).expect("macOS makes a bookmark");
        app.remember_bookmark(path.clone(), data);

        let mut again = App::load(root.join("data")).unwrap();
        let (opened, _) = crate::platform::open_bookmark(&again.bookmarks[&path]).expect("it resolves");
        assert_eq!(std::fs::canonicalize(opened).unwrap(), std::fs::canonicalize(&code).unwrap());

        again.open_bookmarks(|_| Ok((path.clone(), Some(vec![1, 2, 3]))));
        assert_eq!(App::load(root.join("data")).unwrap().bookmarks[&path], vec![1, 2, 3], "the fresh bookmark is saved");

        // once nothing reads the folder, the next launch drops its bookmark
        again.save_settings(Settings::default()).unwrap();
        assert!(App::load(root.join("data")).unwrap().bookmarks.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vacation_is_still_on_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("strays-vac-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let mut app = App::load(dir.clone()).unwrap();
        let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: None, local: None }, Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
        p.adopt(now).unwrap();
        app.pets.push(p);
        app.set_vacation(true, now);
        assert!(App::load(dir.clone()).unwrap().flags.vacation);
        app.set_vacation(false, now);
        assert!(!App::load(dir.clone()).unwrap().flags.vacation);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pets_waking_together_come_back_minutes_apart() {
        let dir = std::env::temp_dir().join(format!("strays-wake-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let mut app = App::load(dir.clone()).unwrap();
        for _ in 0..3 {
            let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: None, local: None }, Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
            p.adopt(now).unwrap();
            p.snooze(now, Some(now + Duration::hours(1))).unwrap();
            app.pets.push(p);
        }
        let at = now + Duration::hours(1);
        assert!(app.tick(at));
        let kinds: Vec<_> = app.pets.iter().map(|p| (p.state.kind, p.state.until)).collect();
        assert_eq!(kinds, vec![(StateKind::Active, None), (StateKind::Snoozed, Some(at + Duration::minutes(3))), (StateKind::Snoozed, Some(at + Duration::minutes(6)))]);
        assert!(app.tick(at + Duration::minutes(3)));
        assert_eq!(app.pets[1].state.kind, StateKind::Active);
        assert_eq!(app.pets[2].state.kind, StateKind::Snoozed);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_project_can_be_adopted_again_once_its_pet_is_gone_for_good() {
        let dir = std::env::temp_dir().join(format!("strays-again-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let mut app = App::load(dir.clone()).unwrap();
        let gh = GithubRef { id: 7, full_name: "me/x".into(), account: "me".into() };
        let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: Some(gh), local: None }, Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
        p.adopt(now).unwrap();
        app.pets.push(p);
        assert!(app.tracked(7));
        app.pets[0].let_go(now, "").unwrap();
        assert!(app.tracked(7), "still undoable");
        app.pets[0].state.since = now - Duration::days(31);
        assert!(!app.tracked(7));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pet_with_a_missing_repo_is_flagged_after_a_restart_until_its_let_go() {
        let dir = std::env::temp_dir().join(format!("strays-missing-{}", Uuid::new_v4()));
        let now = strays_core::chrono::Utc::now();
        let mut app = App::load(dir.clone()).unwrap();
        let gh = GithubRef { id: 7, full_name: "me/x".into(), account: "me".into() };
        let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: Some(gh), local: None }, Activity { last: now, source: ActivitySource::Github }, Pace::Normal);
        p.adopt(now).unwrap();
        p.missing_since = Some(now);
        let id = p.id;
        app.pets.push(p);
        app.save_pet(id);
        let mut app = App::load(dir.clone()).unwrap();
        app.refresh_visible(now);
        assert!(app.views(now)[0].confused);
        assert!(app.issues.iter().any(|i| i.key == "missing" && i.text.contains("Miso")), "{:?}", app.issues);
        app.act(id, Action::LetGo, now).unwrap();
        assert!(!app.issues.iter().any(|i| i.key == "missing"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_crash_is_shown_next_launch_and_reported_only_with_consent() {
        let dir = std::env::temp_dir().join(format!("strays-crash-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let crash = "src/state.rs:42\nno pet called Miso in /Users/me/Code\n\nbacktrace";
        std::fs::write(dir.join(crate::report::CRASH_FILE), crash).unwrap();
        let app = App::load(dir.clone()).unwrap();
        assert!(app.issues.iter().any(|i| i.key == "crash"));
        assert!(app.reports.is_empty(), "reports are off by default");
        assert!(!dir.join(crate::report::CRASH_FILE).exists(), "shown once");

        let mut app = app;
        app.save_settings(Settings { error_reports: true, ..Settings::default() }).unwrap();
        let now = strays_core::chrono::Utc::now();
        let mut p = Pet::new(now, "Miso", Species::Cat, Project::default(), Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
        p.adopt(now).unwrap();
        let id = p.id;
        app.pets.push(p);
        app.save_pet(id);
        std::fs::write(dir.join(crate::report::CRASH_FILE), crash).unwrap();
        let mut app = App::load(dir.clone()).unwrap();
        let r = app.reports.pop().unwrap();
        assert_eq!((r.kind.as_str(), r.message.as_str()), ("crash", "no pet called <name> in <path> at src/state.rs:42"));
        // a repeat of a known problem isn't reported again; a decision is never reported
        app.warn("save:x", "Couldn't save Miso: disk full");
        app.warn("save:x", "Couldn't save Miso: disk full");
        app.warn("action", "All adoption slots are full");
        assert_eq!(app.reports.iter().map(|r| r.message.as_str()).collect::<Vec<_>>(), ["Couldn't save <name>: disk full"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unchanged_folders_are_not_read_again_and_unreadable_ones_are_reported() {
        let dir = std::env::temp_dir().join(format!("strays-local-{}", Uuid::new_v4()));
        let repo = dir.join("proj");
        std::fs::create_dir_all(&repo).unwrap();
        // test fixture only: the app itself never runs git
        let vars = [("GIT_COMMITTER_DATE", "2026-10-01T10:00:00Z"), ("GIT_AUTHOR_DATE", "2026-10-01T10:00:00Z"), ("GIT_CONFIG_GLOBAL", "/dev/null")];
        let git = |args: &[&str]| assert!(std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(&repo).envs(vars).status().unwrap().success());
        git(&["init", "-q", "-b", "main"]);
        git(&["commit", "-q", "--allow-empty", "-m", "a"]);
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let jobs = || vec![(a, repo.display().to_string()), (b, dir.join("gone").display().to_string())];
        let mut seen = Fingerprints::new();
        let first = read_local(jobs(), &mut seen);
        assert_eq!(first.len(), 2);
        assert!(first[0].1.is_ok() && first[1].1.is_err());
        // nothing changed: only the failing folder is tried again
        let again = read_local(jobs(), &mut seen);
        assert_eq!(again.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![b]);
        git(&["commit", "-q", "--allow-empty", "-m", "b"]);
        assert_eq!(read_local(jobs(), &mut seen).len(), 2, "a new commit is read");
        // a pet that no longer reads its folder is forgotten
        read_local(vec![], &mut seen);
        assert!(seen.is_empty());

        let mut app = App::load(dir.join("data")).unwrap();
        let now = strays_core::chrono::Utc::now();
        let local = LocalRef { bookmark_id: Uuid::new_v4(), display_path: "~/gone".into(), path: dir.join("gone").display().to_string() };
        let mut p = Pet::new(now, "Miso", Species::Cat, Project { github: None, local: Some(local) }, Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal);
        p.adopt(now).unwrap();
        let id = p.id;
        app.pets.push(p);
        let results = read_local(app.local_jobs(), &mut seen);
        app.apply_local(results);
        assert!(app.issues.iter().any(|i| i.key == format!("local:{id}") && i.text.contains("~/gone") && i.text.contains("Miso")), "{:?}", app.issues);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn once_closed_for_delete_nothing_writes_the_data_folder() {
        let dir = std::env::temp_dir().join(format!("strays-close-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        app.close();
        std::fs::remove_dir_all(&dir).unwrap();
        app.save_all();
        app.set_account(None);
        app.append_netlog(vec![NetEntry { at: strays_core::chrono::Utc::now(), method: "GET".into(), host: "api.github.com".into(), path: "/".into(), status: 200, reason: "scheduled".into() }]);
        app.save_settings(Settings::default()).unwrap();
        assert!(!dir.exists());
        assert!(app.issues.is_empty(), "skipped on purpose, not failed: {:?}", app.issues);
    }

    #[test]
    fn an_account_saved_by_an_older_version_still_loads() {
        let old: Option<Account> = serde_json::from_str(r#"{"login":"me","expires":null,"repo_count":96}"#).unwrap();
        assert_eq!(old.unwrap().repo_count, 96);
        let now = serde_json::to_value(Account { login: "me".into(), expires: None, repo_count: 1 }).unwrap();
        assert_eq!(now["repoCount"], 1, "the UI reads repoCount");
    }

    #[test]
    fn adoption_refuses_colours_that_arent_hex() {
        let dir = std::env::temp_dir().join(format!("strays-col-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        let req = |k: &str, v: &str| AdoptRequest { repo_id: None, folder: None, species: Species::Cat, colors: [(k.to_string(), v.to_string())].into(), name: String::new(), goal: String::new(), pace: Pace::Normal };
        let now = strays_core::chrono::Utc::now();
        assert_eq!(app.adopt(req("coatBase", "red;x"), now), Err("Those colours aren't valid".into()));
        assert_eq!(app.adopt(req("coat base", "#EFE1CC"), now), Err("Those colours aren't valid".into()));
        assert_eq!(app.adopt(req("coatBase", "#EFE1CC"), now), Err("Pick a repo or a folder".into()), "valid colours pass");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unreadable_file_is_kept_aside_and_reported_not_silently_replaced() {
        let dir = std::env::temp_dir().join(format!("strays-corrupt-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SYNC_FILE), "{half a file").unwrap();
        std::fs::write(dir.join("settings.json"), r#"{"slots": 0}"#).unwrap();
        let mut app = App::load(dir.clone()).unwrap();
        let keys: Vec<&str> = app.issues.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["load:settings.json", "load:sync-state.json"]);
        assert!(app.issues[1].text.contains("sync-state.json.corrupt"), "{}", app.issues[1].text);
        assert_eq!(std::fs::read_to_string(dir.join("sync-state.json.corrupt")).unwrap(), "{half a file");
        assert!(dir.join("settings.json.corrupt").exists());
        // saving now can't destroy the original
        app.save_all();
        assert_eq!(std::fs::read_to_string(dir.join("sync-state.json.corrupt")).unwrap(), "{half a file");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_save_is_shown_and_cleared_once_saving_works_again() {
        let dir = std::env::temp_dir().join(format!("strays-save-{}", Uuid::new_v4()));
        let mut app = App::load(dir.clone()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        app.save_all();
        assert_eq!(app.issues.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["save:sync"]);
        assert!(app.issues[0].text.starts_with("Couldn't save the sync history"));
        std::fs::create_dir_all(dir.join("pets")).unwrap();
        app.save_all();
        assert!(app.issues.is_empty());
        app.warn("x", "first");
        app.warn("x", "second");
        assert_eq!(app.issues, vec![Issue { key: "x".into(), text: "second".into() }], "a repeat replaces, never piles up");
        std::fs::remove_dir_all(dir).unwrap();
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
