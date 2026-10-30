//! User settings the core logic reads (spec §5, §6.3, §7.4). UI-only settings join as the UI is built.
use serde::{Deserialize, Serialize};

use crate::mood::{Pace, Thresholds};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Liveliness {
    Calm,
    Gentle,
    Lively,
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
        Ok(())
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
