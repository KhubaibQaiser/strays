//! User settings the core logic reads (spec §5, §6.3, §7.4). UI-only settings join as the UI is built.
use serde::{Deserialize, Serialize};

use crate::mood::{Pace, Thresholds};
use crate::pet::Species;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Liveliness {
    Calm,
    Gentle,
    Lively,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PetSize {
    Small,
    Medium,
    Large,
}

impl PetSize {
    /// Pet height in points (§7.1).
    pub fn height(self) -> f64 {
        match self {
            PetSize::Small => 72.0,
            PetSize::Medium => 104.0,
            PetSize::Large => 136.0,
        }
    }
}

/// The key held to interact with pets (§7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Modifier {
    Option,
    Control,
    Shift,
    OptionShift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NudgeStyle {
    Gentle,
    Playful,
    Off,
}

/// Minutes between automatic syncs; `None` = manual only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncInterval(pub Option<u32>);

impl SyncInterval {
    pub const CHOICES: [Option<u32>; 7] = [Some(5), Some(15), Some(30), Some(60), Some(180), Some(720), None];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub thresholds: Thresholds,
    pub default_pace: Pace,
    pub skip_weekends: bool,
    pub slots: u32,
    pub holiday_days: u32,
    pub release_cooldown_days: u32,
    pub let_go_undo_days: u32,
    pub sync_interval: SyncInterval,
    pub sync_on_wake: bool,
    pub sync_on_launch: bool,
    pub sync_only_on_wifi: bool,
    pub sync_pause_on_low_power: bool,
    pub max_visible: u32,
    pub min_visible: u32,
    pub rotation_hours: u32,
    pub liveliness: Liveliness,
    /// Repo names (case-insensitive, `*` wildcard) never offered as new projects.
    pub ignore: Vec<String>,
    pub pet_size: PetSize,
    pub modifier: Modifier,
    pub hide_in_full_screen: bool,
    pub nudge_style: NudgeStyle,
    pub nudges_per_pet_daily: u32,
    pub nudges_daily: u32,
    pub quiet_hours: bool,
    /// Minutes after local midnight; the range may wrap past midnight.
    pub quiet_start: u32,
    pub quiet_end: u32,
    /// Minutes between pets waking together (§5.6).
    pub wake_stagger: u32,
    /// Parent folders watched for new git repos (§5.9).
    pub watched_folders: Vec<String>,
    /// No network at all, GitHub included (§8.5 "Go fully offline").
    pub offline: bool,
    /// Sound (§8.4): off by default; nothing depends on it.
    pub sound: bool,
    /// 0–100.
    pub volume: u32,
    pub muted_species: Vec<Species>,
    /// Opt-in update check (§16): off by default; weekly while on, anonymous, to api.github.com only.
    pub check_updates: bool,
    /// Opt-in error and crash reports to PostHog (`report.rs`): off until the user says yes.
    pub error_reports: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::default(),
            default_pace: Pace::Normal,
            skip_weekends: false,
            slots: 3,
            holiday_days: 3,
            release_cooldown_days: 14,
            let_go_undo_days: 30,
            sync_interval: SyncInterval(Some(30)),
            sync_on_wake: true,
            sync_on_launch: true,
            sync_only_on_wifi: false,
            sync_pause_on_low_power: true,
            max_visible: 3,
            min_visible: 1,
            rotation_hours: 3,
            liveliness: Liveliness::Calm,
            ignore: Vec::new(),
            pet_size: PetSize::Medium,
            modifier: Modifier::Option,
            hide_in_full_screen: true,
            nudge_style: NudgeStyle::Gentle,
            nudges_per_pet_daily: 1,
            nudges_daily: 3,
            quiet_hours: true,
            quiet_start: 22 * 60,
            quiet_end: 8 * 60,
            wake_stagger: 3,
            watched_folders: Vec::new(),
            offline: false,
            sound: false,
            volume: 50,
            muted_species: Vec::new(),
            check_updates: false,
            error_reports: false,
        }
    }
}

impl Settings {
    /// Rejects values the UI would never produce, so a hand-edited or corrupt file can't break the logic.
    pub fn validate(&self) -> Result<(), String> {
        let check = |ok: bool, what: &str| if ok { Ok(()) } else { Err(format!("invalid setting: {what}")) };
        check(Thresholds::new(self.thresholds.0).is_some(), "thresholds")?;
        check((1..=20).contains(&self.slots), "slots (1–20)")?;
        check((1..=14).contains(&self.holiday_days), "holidayDays (1–14)")?;
        check((1..=8).contains(&self.max_visible), "maxVisible (1–8)")?;
        check(self.min_visible <= self.max_visible, "minVisible (0–maxVisible)")?;
        check(self.rotation_hours >= 1, "rotationHours")?;
        check(SyncInterval::CHOICES.contains(&self.sync_interval.0), "syncInterval")?;
        check(self.nudges_per_pet_daily <= 10 && self.nudges_daily <= 20, "nudge caps")?;
        check(self.quiet_start < 1440 && self.quiet_end < 1440, "quiet hours")?;
        check(self.wake_stagger <= 60, "wakeStagger (0–60)")?;
        check(self.volume <= 100, "volume (0–100)")?;
        Ok(())
    }

    /// Whether `minute` (after local midnight) falls in quiet hours. The range may wrap past midnight.
    pub fn is_quiet(&self, minute: u32) -> bool {
        let (s, e) = (self.quiet_start, self.quiet_end);
        self.quiet_hours && if s <= e { (s..e).contains(&minute) } else { minute >= s || minute < e }
    }

    pub fn ignored(&self, repo_name: &str) -> bool {
        self.ignore.iter().any(|p| glob_match(&p.to_lowercase(), &repo_name.to_lowercase()))
    }
}

/// `*` matches any run of characters; everything else is literal.
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text[first.len()..].ends_with(last) || text.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let s = Settings::default();
        s.validate().unwrap();
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        // missing fields fall back to defaults, so older files keep loading
        let partial: Settings = serde_json::from_str(r#"{"slots": 5}"#).unwrap();
        assert_eq!(partial.slots, 5);
        assert_eq!(partial.holiday_days, 3);
    }

    #[test]
    fn rejects_bad_values() {
        let bad = Settings { slots: 0, ..Settings::default() };
        assert!(bad.validate().is_err());
        let bad = Settings { min_visible: 4, ..Settings::default() };
        assert!(bad.validate().is_err());
        let bad = Settings { sync_interval: SyncInterval(Some(7)), ..Settings::default() };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        let s = Settings::default(); // 22:00–08:00
        assert!(s.is_quiet(23 * 60) && s.is_quiet(0) && s.is_quiet(7 * 60 + 59));
        assert!(!s.is_quiet(8 * 60) && !s.is_quiet(12 * 60) && !s.is_quiet(21 * 60 + 59));
        let day = Settings { quiet_start: 9 * 60, quiet_end: 17 * 60, ..Settings::default() };
        assert!(day.is_quiet(12 * 60) && !day.is_quiet(18 * 60));
        assert!(!Settings { quiet_hours: false, ..Settings::default() }.is_quiet(23 * 60));
    }

    #[test]
    fn ignore_patterns() {
        let s = Settings { ignore: vec!["test-*".into(), "*-archive".into(), "dotfiles".into(), "a*b*c".into()], ..Settings::default() };
        assert!(s.ignored("Test-Repo"));
        assert!(s.ignored("old-archive"));
        assert!(s.ignored("dotfiles"));
        assert!(s.ignored("axxbyyc"));
        assert!(!s.ignored("dotfiles2"));
        assert!(!s.ignored("acb"));
        assert!(!s.ignored("strays"));
    }
}
