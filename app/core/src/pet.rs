//! Pet record (spec §11.1) and its state machine (§5.1, §5.5, §5.6).
use chrono::{Duration, TimeZone};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::clock::{Interval, Time};
use crate::mood::{self, Mood, Pace, Thresholds};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Species {
    Cat,
    Dog,
    Hamster,
    Parrot,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRef {
    pub id: u64,
    pub full_name: String,
    pub account: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRef {
    pub bookmark_id: Uuid,
    pub display_path: String,
    /// Absolute path. ponytail: becomes a security-scoped bookmark when the App Sandbox is on (Gate 4).
    #[serde(default)]
    pub path: String,
}

/// "What does done look like?" (§8.2), shown and ticked on the info card.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub text: String,
    pub checklist: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub github: Option<GithubRef>,
    pub local: Option<LocalRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StateKind {
    Shelter,
    Active,
    Snoozed,
    Paused,
    Holiday,
    Graduated,
    LetGo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub kind: StateKind,
    /// When a snooze, holiday or scheduled pause resume ends.
    pub until: Option<Time>,
    pub since: Time,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivitySource {
    Github,
    LocalCommit,
    LocalRemoteRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub last: Time,
    pub source: ActivitySource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrozenWhy {
    Holiday,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frozen {
    #[serde(flatten)]
    pub span: Interval,
    pub why: FrozenWhy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub at: Time,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pet {
    pub schema_version: u32,
    pub id: Uuid,
    pub created_at: Time,
    pub name: String,
    pub species: Species,
    /// Rive view-model colour name → "#RRGGBB".
    pub colors: std::collections::BTreeMap<String, String>,
    pub accessory: Option<String>,
    pub project: Project,
    pub pace: Pace,
    pub thresholds_override: Option<Thresholds>,
    pub state: State,
    pub activity: Activity,
    pub frozen: Vec<Frozen>,
    pub releases: Vec<Release>,
    #[serde(default)]
    pub goal: Goal,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The action doesn't apply in the pet's current state.
    WrongState(StateKind),
    UndoWindowPassed,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::WrongState(k) => {
                let now = match k {
                    StateKind::Shelter => "in the shelter",
                    StateKind::Active => "awake",
                    StateKind::Snoozed => "snoozing",
                    StateKind::Paused => "paused",
                    StateKind::Holiday => "on holiday",
                    StateKind::Graduated => "graduated",
                    StateKind::LetGo => "let go",
                };
                write!(f, "Not while the pet is {now}")
            }
            Refused::UndoWindowPassed => f.write_str("Too late to undo: the undo period has passed"),
        }
    }
}

/// What a release did: a holiday, or only a celebration (§5.5).
#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseOutcome {
    Holiday { until: Time },
    CelebrationOnly { reason: &'static str },
}

pub const SCHEMA_VERSION: u32 = 1;

/// Graduation rewards (spec §10.2), in the order of the rigs' `accessory` number (1-5).
pub const ACCESSORIES: [&str; 5] = ["cap", "scarf", "bow", "star", "crown"];

impl Pet {
    pub fn new(now: Time, name: &str, species: Species, project: Project, activity: Activity, pace: Pace) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4(),
            created_at: now,
            name: name.into(),
            species,
            colors: Default::default(),
            accessory: None,
            project,
            pace,
            thresholds_override: None,
            state: State { kind: StateKind::Shelter, until: None, since: now, reason: String::new() },
            activity,
            frozen: Vec::new(),
            releases: Vec::new(),
            goal: Goal::default(),
        }
    }

    fn set_state(&mut self, kind: StateKind, until: Option<Time>, now: Time, reason: &str) {
        self.state = State { kind, until, since: now, reason: reason.into() };
    }

    fn require(&self, kinds: &[StateKind]) -> Result<(), Refused> {
        if kinds.contains(&self.state.kind) { Ok(()) } else { Err(Refused::WrongState(self.state.kind)) }
    }

    /// Records activity; `last` never decreases (§5.4).
    pub fn record_activity(&mut self, at: Time, source: ActivitySource) {
        if at > self.activity.last {
            self.activity = Activity { last: at, source };
        }
    }

    pub fn is_github_backed(&self) -> bool {
        self.project.github.is_some()
    }

    pub fn thresholds(&self, settings: &Settings) -> Thresholds {
        self.thresholds_override.unwrap_or_else(|| settings.thresholds.scaled(self.pace))
    }

    /// `last_check` is the last successful GitHub check (§5.3); ignored for local-only pets.
    pub fn idle_days<Tz: TimeZone>(&self, now: Time, last_check: Option<Time>, settings: &Settings, tz: &Tz) -> f64 {
        let eval = if self.is_github_backed() { last_check.unwrap_or(self.activity.last).min(now) } else { now };
        let frozen: Vec<Interval> = self.frozen.iter().map(|f| f.span).collect();
        mood::idle_days(self.activity.last, eval, &frozen, settings.skip_weekends, tz)
    }

    pub fn mood<Tz: TimeZone>(&self, now: Time, last_check: Option<Time>, settings: &Settings, tz: &Tz) -> Mood {
        self.thresholds(settings).mood(self.idle_days(now, last_check, settings, tz))
    }

    /// Shelter → Active. The caller checks for a free slot (§5.8).
    pub fn adopt(&mut self, now: Time) -> Result<(), Refused> {
        self.require(&[StateKind::Shelter])?;
        self.set_state(StateKind::Active, None, now, "");
        Ok(())
    }

    /// Active → Snoozed. Hunger keeps counting. `until: None` = until the user wakes it.
    pub fn snooze(&mut self, now: Time, until: Option<Time>) -> Result<(), Refused> {
        self.require(&[StateKind::Active])?;
        self.set_state(StateKind::Snoozed, until, now, "");
        Ok(())
    }

    pub fn wake(&mut self, now: Time) -> Result<(), Refused> {
        self.require(&[StateKind::Snoozed])?;
        self.set_state(StateKind::Active, None, now, "");
        Ok(())
    }

    /// Active or Snoozed → Paused. Hunger is frozen until resumed.
    pub fn pause(&mut self, now: Time, resume_at: Option<Time>) -> Result<(), Refused> {
        self.require(&[StateKind::Active, StateKind::Snoozed])?;
        self.frozen.push(Frozen { span: Interval { from: now, to: None }, why: FrozenWhy::Pause });
        self.set_state(StateKind::Paused, resume_at, now, "");
        Ok(())
    }

    pub fn resume(&mut self, now: Time) -> Result<(), Refused> {
        self.require(&[StateKind::Paused])?;
        for f in self.frozen.iter_mut().filter(|f| f.why == FrozenWhy::Pause && f.span.to.is_none()) {
            f.span.to = Some(now);
        }
        self.set_state(StateKind::Active, None, now, "");
        Ok(())
    }

    /// Records the release; sends the pet on holiday only if it's eligible (§5.5): new activity since
    /// the previous release, and the last holiday started at least the cooldown ago.
    pub fn mark_release(&mut self, now: Time, label: Option<String>, settings: &Settings) -> Result<ReleaseOutcome, Refused> {
        self.require(&[StateKind::Active])?;
        let new_work = self.releases.last().is_none_or(|r| self.activity.last > r.at);
        let cooldown = Duration::days(settings.release_cooldown_days.into());
        let rested = self
            .frozen
            .iter()
            .rfind(|f| f.why == FrozenWhy::Holiday)
            .is_none_or(|f| now - f.span.from >= cooldown);
        self.releases.push(Release { at: now, label });
        if !new_work {
            return Ok(ReleaseOutcome::CelebrationOnly { reason: "No new work since the last release" });
        }
        if !rested {
            return Ok(ReleaseOutcome::CelebrationOnly { reason: "Back from a holiday recently" });
        }
        let until = now + Duration::days(settings.holiday_days.into());
        self.frozen.push(Frozen { span: Interval { from: now, to: Some(until) }, why: FrozenWhy::Holiday });
        self.set_state(StateKind::Holiday, Some(until), now, "");
        Ok(ReleaseOutcome::Holiday { until })
    }

    /// Finished project (user, or repo archived on GitHub). Final. The pet earns one of the
    /// graduation accessories (§5.8), picked from its id so it never changes.
    pub fn graduate(&mut self, now: Time, reason: &str) -> Result<(), Refused> {
        self.require(&[StateKind::Active, StateKind::Snoozed, StateKind::Paused, StateKind::Holiday, StateKind::Shelter])?;
        self.close_open_frozen(now);
        self.accessory = Some(ACCESSORIES[(self.id.as_u128() % ACCESSORIES.len() as u128) as usize].into());
        self.set_state(StateKind::Graduated, None, now, reason);
        Ok(())
    }

    /// User choice, or repo deleted. Undoable for `let_go_undo_days`.
    pub fn let_go(&mut self, now: Time, reason: &str) -> Result<(), Refused> {
        self.require(&[StateKind::Active, StateKind::Snoozed, StateKind::Paused, StateKind::Holiday, StateKind::Shelter])?;
        self.close_open_frozen(now);
        self.set_state(StateKind::LetGo, None, now, reason);
        Ok(())
    }

    /// Back to the shelter, so adopting it again respects the slot limit.
    pub fn undo_let_go(&mut self, now: Time, settings: &Settings) -> Result<(), Refused> {
        self.require(&[StateKind::LetGo])?;
        if now - self.state.since > Duration::days(settings.let_go_undo_days.into()) {
            return Err(Refused::UndoWindowPassed);
        }
        self.set_state(StateKind::Shelter, None, now, "");
        Ok(())
    }

    fn close_open_frozen(&mut self, now: Time) {
        for f in self.frozen.iter_mut() {
            if f.span.to.is_none_or(|to| to > now) {
                f.span.to = Some(now);
            }
        }
    }

    /// Ends snoozes, holidays and scheduled pauses whose time has come. Returns true if the state changed.
    /// Called on start, on wake and from timers computed from `next_deadline` (§11: no long sleeps).
    pub fn tick(&mut self, now: Time) -> bool {
        match (self.state.kind, self.state.until) {
            (StateKind::Snoozed | StateKind::Holiday, Some(until)) if until <= now => {
                self.set_state(StateKind::Active, None, until, "");
                true
            }
            (StateKind::Paused, Some(until)) if until <= now => {
                self.resume(until).expect("paused");
                true
            }
            _ => false,
        }
    }

    pub fn next_deadline(&self) -> Option<Time> {
        match self.state.kind {
            StateKind::Snoozed | StateKind::Holiday | StateKind::Paused => self.state.until,
            _ => None,
        }
    }
}

/// Number of pets using an adoption slot: everything that isn't in the shelter or finished.
pub fn slots_used(pets: &[Pet]) -> usize {
    pets.iter()
        .filter(|p| !matches!(p.state.kind, StateKind::Shelter | StateKind::Graduated | StateKind::LetGo))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn t(s: &str) -> Time {
        s.parse().unwrap()
    }

    fn pet(now: Time) -> Pet {
        let gh = GithubRef { id: 1, full_name: "me/app".into(), account: "me".into() };
        let mut p = Pet::new(
            now,
            "Miso",
            Species::Cat,
            Project { github: Some(gh), local: None },
            Activity { last: now, source: ActivitySource::Github },
            Pace::Normal,
        );
        p.adopt(now).unwrap();
        p
    }

    #[test]
    fn activity_never_decreases() {
        let now = t("2026-10-10T00:00:00Z");
        let mut p = pet(now);
        p.record_activity(t("2026-10-01T00:00:00Z"), ActivitySource::LocalCommit);
        assert_eq!(p.activity.last, now);
        p.record_activity(t("2026-10-11T00:00:00Z"), ActivitySource::LocalCommit);
        assert_eq!(p.activity.source, ActivitySource::LocalCommit);
    }

    #[test]
    fn failed_sync_freezes_mood_for_github_pets() {
        let s = Settings::default();
        let start = t("2026-10-01T00:00:00Z");
        let p = pet(start);
        let last_check = Some(t("2026-10-03T00:00:00Z"));
        // a month later, no successful check since 10-03: still 2 idle days
        assert_eq!(p.idle_days(t("2026-11-01T00:00:00Z"), last_check, &s, &Utc), 2.0);
        assert_eq!(p.mood(t("2026-11-01T00:00:00Z"), last_check, &s, &Utc), Mood::Joyful);
        // a local-only pet uses now
        let mut local = p.clone();
        local.project.github = None;
        assert_eq!(local.mood(t("2026-11-01T00:00:00Z"), last_check, &s, &Utc), Mood::Gloomy);
    }

    #[test]
    fn snooze_keeps_counting_pause_freezes() {
        let s = Settings::default();
        let start = t("2026-10-01T00:00:00Z");
        let mut p = pet(start);
        p.project.github = None;
        p.snooze(start, Some(t("2026-10-04T00:00:00Z"))).unwrap();
        assert!(p.tick(t("2026-10-05T00:00:00Z")));
        assert_eq!(p.state.kind, StateKind::Active);
        assert_eq!(p.state.since, t("2026-10-04T00:00:00Z"));
        assert_eq!(p.idle_days(t("2026-10-05T00:00:00Z"), None, &s, &Utc), 4.0);

        p.pause(t("2026-10-05T00:00:00Z"), None).unwrap();
        assert_eq!(p.idle_days(t("2026-10-20T00:00:00Z"), None, &s, &Utc), 4.0);
        p.resume(t("2026-10-20T00:00:00Z")).unwrap();
        assert_eq!(p.idle_days(t("2026-10-21T00:00:00Z"), None, &s, &Utc), 5.0);
    }

    #[test]
    fn scheduled_resume() {
        let start = t("2026-10-01T00:00:00Z");
        let mut p = pet(start);
        p.pause(start, Some(t("2026-10-08T00:00:00Z"))).unwrap();
        assert_eq!(p.next_deadline(), Some(t("2026-10-08T00:00:00Z")));
        assert!(!p.tick(t("2026-10-07T00:00:00Z")));
        assert!(p.tick(t("2026-10-09T00:00:00Z")));
        assert_eq!(p.frozen[0].span.to, Some(t("2026-10-08T00:00:00Z")));
    }

    #[test]
    fn release_rules() {
        let s = Settings::default();
        let d0 = t("2026-10-01T00:00:00Z");
        let mut p = pet(d0);
        let out = p.mark_release(d0, Some("v1".into()), &s).unwrap();
        assert_eq!(out, ReleaseOutcome::Holiday { until: t("2026-10-04T00:00:00Z") });
        assert_eq!(p.mark_release(d0, None, &s), Err(Refused::WrongState(StateKind::Holiday)));
        assert!(p.tick(t("2026-10-04T00:00:00Z")));

        // no new work since v1
        let out = p.mark_release(t("2026-10-05T00:00:00Z"), None, &s).unwrap();
        assert!(matches!(out, ReleaseOutcome::CelebrationOnly { .. }));
        // new work, but within 14 days of the last holiday
        p.record_activity(t("2026-10-06T00:00:00Z"), ActivitySource::Github);
        let out = p.mark_release(t("2026-10-07T00:00:00Z"), None, &s).unwrap();
        assert_eq!(out, ReleaseOutcome::CelebrationOnly { reason: "Back from a holiday recently" });
        // new work after the cooldown
        p.record_activity(t("2026-10-15T00:00:00Z"), ActivitySource::Github);
        let out = p.mark_release(t("2026-10-15T00:00:00Z"), Some("v2".into()), &s).unwrap();
        assert!(matches!(out, ReleaseOutcome::Holiday { .. }));
        assert_eq!(p.releases.len(), 4);
    }

    #[test]
    fn holiday_does_not_count_as_idle() {
        let s = Settings::default();
        let d0 = t("2026-10-01T00:00:00Z");
        let mut p = pet(d0);
        p.project.github = None;
        p.mark_release(d0, None, &s).unwrap();
        p.tick(t("2026-10-04T00:00:00Z"));
        assert_eq!(p.idle_days(t("2026-10-05T00:00:00Z"), None, &s, &Utc), 1.0);
    }

    #[test]
    fn let_go_and_undo() {
        let s = Settings::default();
        let d0 = t("2026-10-01T00:00:00Z");
        let mut p = pet(d0);
        p.pause(d0, None).unwrap();
        p.let_go(t("2026-10-02T00:00:00Z"), "Repo no longer exists").unwrap();
        assert_eq!(p.frozen[0].span.to, Some(t("2026-10-02T00:00:00Z")));
        assert_eq!(p.undo_let_go(t("2026-11-15T00:00:00Z"), &s), Err(Refused::UndoWindowPassed));
        p.undo_let_go(t("2026-10-20T00:00:00Z"), &s).unwrap();
        assert_eq!(p.state.kind, StateKind::Shelter);
        assert_eq!(p.wake(d0), Err(Refused::WrongState(StateKind::Shelter)));
    }

    #[test]
    fn slots_count_only_living_pets() {
        let d0 = t("2026-10-01T00:00:00Z");
        let mut pets = vec![pet(d0), pet(d0), pet(d0), pet(d0)];
        pets[1].snooze(d0, None).unwrap();
        pets[2].graduate(d0, "").unwrap();
        pets[3].state.kind = StateKind::Shelter;
        assert_eq!(slots_used(&pets), 2);
    }

    #[test]
    fn graduating_earns_a_fixed_accessory() {
        let d0 = t("2026-10-01T00:00:00Z");
        let mut p = pet(d0);
        assert_eq!(p.accessory, None);
        p.graduate(d0, "").unwrap();
        let earned = p.accessory.clone().unwrap();
        assert!(ACCESSORIES.contains(&earned.as_str()));
        let mut twin = pet(d0);
        twin.id = p.id;
        twin.graduate(d0, "").unwrap();
        assert_eq!(twin.accessory, Some(earned));
    }

    #[test]
    fn json_round_trip_uses_spec_names() {
        let p = pet(t("2026-10-01T00:00:00Z"));
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["state"]["kind"], "active");
        assert_eq!(json["project"]["github"]["fullName"], "me/app");
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(serde_json::from_value::<Pet>(json).unwrap(), p);
    }

    #[test]
    fn refusals_read_as_sentences() {
        assert_eq!(Refused::WrongState(StateKind::Holiday).to_string(), "Not while the pet is on holiday");
        assert_eq!(crate::github::GhError::Http(502).to_string(), "GitHub is having trouble (502)");
    }
}
