//! Nudges (spec §5.10): a hungry or gloomy pet now and then says something. Rare by design:
//! daily caps per pet and overall, an hour between any two, and never when it would intrude.
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::clock::Time;
use crate::mood::Mood;
use crate::settings::{NudgeStyle, Settings};

const MIN_GAP_MINUTES: i64 = 60;
const BREAK_PROMPT_EVERY_DAYS: i64 = 7;

/// What's been said, saved as `nudges.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NudgeLog {
    pub day: Option<NaiveDate>,
    pub today_total: u32,
    pub today_per_pet: std::collections::BTreeMap<Uuid, u32>,
    pub last_at: Option<Time>,
    pub last_break_prompt: Option<Time>,
}

/// One on-screen pet the nudger may pick.
pub struct Candidate<'a> {
    pub id: Uuid,
    pub repo: Option<&'a str>,
    pub mood: Mood,
}

/// Why a nudge can't happen right now, whatever the pets' moods.
pub struct Context {
    pub now: Time,
    pub local_day: NaiveDate,
    pub local_minute: u32,
    pub presentation: bool,
    pub full_screen: bool,
}

#[derive(Debug, PartialEq)]
pub enum Nudge {
    Pet { id: Uuid, text: String },
    /// Everyone is hungry: offer to pause them all (§5.10).
    TakingABreak { id: Uuid, text: String },
}

/// Picks at most one nudge and records it in `log`. `all_active` are the moods of every Active pet,
/// on screen or not, for the "everyone's idle" prompt.
pub fn pick(on_screen: &[Candidate], all_active: &[Mood], log: &mut NudgeLog, s: &Settings, cx: &Context) -> Option<Nudge> {
    if log.day != Some(cx.local_day) {
        log.day = Some(cx.local_day);
        log.today_total = 0;
        log.today_per_pet.clear();
    }
    let blocked = s.nudge_style == NudgeStyle::Off || cx.presentation || cx.full_screen || s.is_quiet(cx.local_minute);
    let too_soon = log.last_at.is_some_and(|t| cx.now - t < Duration::minutes(MIN_GAP_MINUTES));
    if blocked || too_soon || log.today_total >= s.nudges_daily {
        return None;
    }
    let speaker = on_screen
        .iter()
        .filter(|c| c.mood >= Mood::Hungry && log.today_per_pet.get(&c.id).copied().unwrap_or(0) < s.nudges_per_pet_daily)
        .max_by_key(|c| c.mood)?;

    let everyone_hungry = all_active.len() >= 2 && all_active.iter().all(|m| *m >= Mood::Hungry);
    let break_due = log.last_break_prompt.is_none_or(|t| cx.now - t >= Duration::days(BREAK_PROMPT_EVERY_DAYS));
    let nudge = if everyone_hungry && break_due {
        log.last_break_prompt = Some(cx.now);
        Nudge::TakingABreak { id: speaker.id, text: "Taking a break? You can pause everyone from the menu bar.".into() }
    } else {
        Nudge::Pet { id: speaker.id, text: line(speaker, s.nudge_style, cx.now) }
    };
    log.last_at = Some(cx.now);
    log.today_total += 1;
    *log.today_per_pet.entry(speaker.id).or_default() += 1;
    Some(nudge)
}

/// Never guilt-tripping (§10.5): an invitation, not a reproach.
fn line(c: &Candidate, style: NudgeStyle, now: Time) -> String {
    let project = c.repo.map(|r| r.rsplit('/').next().unwrap_or(r)).unwrap_or("our project");
    let gentle = [
        format!("Shall we look at {project} today?"),
        format!("A tiny commit on {project} would make my day."),
        format!("I've been thinking about {project}."),
    ];
    let playful = [
        format!("*nudges {project} towards you*"),
        format!("{project} misses you. So do I. Mostly {project}."),
        format!("Five minutes on {project}? I'll keep watch."),
    ];
    let lines = if style == NudgeStyle::Playful { &playful } else { &gentle };
    lines[(now.timestamp() as usize / 60) % lines.len()].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Time {
        s.parse().unwrap()
    }

    fn cx(now: &str, minute: u32) -> Context {
        let now = t(now);
        Context { now, local_day: now.date_naive(), local_minute: minute, presentation: false, full_screen: false }
    }

    #[test]
    fn hungry_pet_speaks_within_caps_and_gaps() {
        let s = Settings::default(); // 1 per pet, 3 a day, quiet 22:00–08:00
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let pets = [
            Candidate { id: a, repo: Some("me/strays"), mood: Mood::Hungry },
            Candidate { id: b, repo: None, mood: Mood::Gloomy },
        ];
        let mut log = NudgeLog::default();
        let moods = [Mood::Hungry, Mood::Gloomy, Mood::Content];

        let first = pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T10:00:00Z", 600)).unwrap();
        assert!(matches!(first, Nudge::Pet { id, .. } if id == b), "the gloomiest speaks first");
        assert_eq!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T10:30:00Z", 630)), None, "an hour between nudges");
        let second = pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T11:00:00Z", 660)).unwrap();
        assert!(matches!(second, Nudge::Pet { id, ref text } if id == a && text.contains("strays")));
        assert_eq!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T12:00:00Z", 720)), None, "each pet once a day");
        // a new day resets the counts
        assert!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-02T10:00:00Z", 600)).is_some());
    }

    #[test]
    fn never_when_it_would_intrude() {
        let pets = [Candidate { id: Uuid::new_v4(), repo: None, mood: Mood::Gloomy }];
        let moods = [Mood::Gloomy];
        let s = Settings::default();
        let mut log = NudgeLog::default();
        assert_eq!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T23:00:00Z", 23 * 60)), None, "quiet hours");
        let mut c = cx("2026-11-01T10:00:00Z", 600);
        c.presentation = true;
        assert_eq!(pick(&pets, &moods, &mut log, &s, &c), None, "presentation mode");
        c.presentation = false;
        c.full_screen = true;
        assert_eq!(pick(&pets, &moods, &mut log, &s, &c), None, "full-screen app");
        let off = Settings { nudge_style: NudgeStyle::Off, ..Settings::default() };
        assert_eq!(pick(&pets, &moods, &mut log, &off, &cx("2026-11-01T10:00:00Z", 600)), None, "style off");
        let content = [Candidate { id: Uuid::new_v4(), repo: None, mood: Mood::Peckish }];
        assert_eq!(pick(&content, &[Mood::Peckish], &mut log, &s, &cx("2026-11-01T10:00:00Z", 600)), None, "only hungry pets speak");
        assert_eq!(log.today_total, 0);
    }

    #[test]
    fn everyone_hungry_offers_a_break_once_a_week() {
        let s = Settings { nudges_per_pet_daily: 5, nudges_daily: 10, ..Settings::default() };
        let pets = [Candidate { id: Uuid::new_v4(), repo: None, mood: Mood::Hungry }];
        let moods = [Mood::Hungry, Mood::Gloomy];
        let mut log = NudgeLog::default();
        assert!(matches!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T10:00:00Z", 600)), Some(Nudge::TakingABreak { .. })));
        assert!(matches!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-01T12:00:00Z", 720)), Some(Nudge::Pet { .. })));
        assert!(matches!(pick(&pets, &moods, &mut log, &s, &cx("2026-11-08T12:00:00Z", 720)), Some(Nudge::TakingABreak { .. })));
    }
}
