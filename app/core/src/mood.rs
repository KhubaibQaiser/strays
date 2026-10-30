//! Mood from idle days (spec §5.2, §5.3).
use chrono::{Datelike, Duration, NaiveTime, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

use crate::clock::{Interval, Time};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mood {
    Joyful = 1,
    Content = 2,
    Peckish = 3,
    Hungry = 4,
    Gloomy = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pace {
    Sprint,
    Normal,
    SlowBurn,
}

impl Pace {
    fn factor(self) -> f64 {
        match self {
            Pace::Sprint => 0.5,
            Pace::Normal => 1.0,
            Pace::SlowBurn => 2.0,
        }
    }
}

/// Upper bounds in days for Joyful, Content, Peckish, Hungry; beyond the last is Gloomy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds(pub [f64; 4]);

impl Default for Thresholds {
    fn default() -> Self {
        Self([2.0, 5.0, 10.0, 20.0])
    }
}

impl Thresholds {
    /// Thresholds must be positive and strictly increasing (the settings UI enforces the same).
    pub fn new(days: [f64; 4]) -> Option<Self> {
        let ok = days[0] > 0.0 && days.windows(2).all(|w| w[0] < w[1]) && days.iter().all(|d| d.is_finite());
        ok.then_some(Self(days))
    }

    pub fn scaled(self, pace: Pace) -> Self {
        Self(self.0.map(|d| d * pace.factor()))
    }

    pub fn mood(self, idle_days: f64) -> Mood {
        let moods = [Mood::Joyful, Mood::Content, Mood::Peckish, Mood::Hungry];
        moods
            .into_iter()
            .zip(self.0)
            .find(|&(_, bound)| idle_days <= bound)
            .map_or(Mood::Gloomy, |(m, _)| m)
    }
}

/// Idle days per the fairness rule (§5.3): time since the last activity, minus frozen (holiday, pause)
/// time and, optionally, weekends in local time `tz`. Overlapping exclusions are counted once.
/// `eval_time` is `now` for local-only pets and the last successful check for GitHub-backed pets,
/// so a failed sync can never make the mood worse. A clock that went backwards gives 0.
pub fn idle_days<Tz: TimeZone>(last_activity: Time, eval_time: Time, frozen: &[Interval], skip_weekends: bool, tz: &Tz) -> f64 {
    if eval_time <= last_activity {
        return 0.0;
    }
    let clip = |from: Time, to: Time| (from.max(last_activity), to.min(eval_time));
    let mut excluded: Vec<(Time, Time)> = frozen
        .iter()
        .map(|i| clip(i.from, i.to.unwrap_or(eval_time)))
        .collect();
    if skip_weekends {
        excluded.extend(weekends(last_activity, eval_time, tz).into_iter().map(|(a, b)| clip(a, b)));
    }
    excluded.retain(|(a, b)| a < b);
    excluded.sort_by_key(|&(a, _)| a);

    let mut total = Duration::zero();
    let mut current: Option<(Time, Time)> = None;
    for (a, b) in excluded {
        current = match current {
            Some((ca, cb)) if a <= cb => Some((ca, cb.max(b))),
            Some((ca, cb)) => {
                total += cb - ca;
                Some((a, b))
            }
            None => Some((a, b)),
        };
    }
    if let Some((ca, cb)) = current {
        total += cb - ca;
    }
    let idle = (eval_time - last_activity) - total;
    (idle.num_seconds().max(0) as f64) / 86_400.0
}

/// Saturday 00:00 to Monday 00:00 local, for every weekend touching [from, to].
fn weekends<Tz: TimeZone>(from: Time, to: Time, tz: &Tz) -> Vec<(Time, Time)> {
    let midnight = |d: chrono::NaiveDate| -> Time {
        let naive = d.and_time(NaiveTime::MIN);
        // A DST change at midnight can skip or repeat it; any nearby instant is fine for whole days.
        let local = tz
            .from_local_datetime(&naive)
            .earliest()
            .or_else(|| tz.from_local_datetime(&(naive + Duration::hours(1))).earliest())
            .expect("local midnight or 01:00 exists");
        local.with_timezone(&chrono::Utc)
    };
    let mut out = Vec::new();
    let mut day = from.with_timezone(tz).date_naive();
    // step back to this weekend's Saturday so a span starting on Sunday is covered
    while day.weekday() != Weekday::Sat {
        day = day.pred_opt().unwrap();
    }
    let end = to.with_timezone(tz).date_naive();
    while day <= end {
        out.push((midnight(day), midnight(day + Duration::days(2))));
        day += Duration::days(7);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};
    use proptest::prelude::*;

    fn t(s: &str) -> Time {
        s.parse().unwrap()
    }

    #[test]
    fn default_thresholds_map_to_moods() {
        let th = Thresholds::default();
        assert_eq!(th.mood(0.0), Mood::Joyful);
        assert_eq!(th.mood(2.0), Mood::Joyful);
        assert_eq!(th.mood(2.1), Mood::Content);
        assert_eq!(th.mood(10.0), Mood::Peckish);
        assert_eq!(th.mood(19.9), Mood::Hungry);
        assert_eq!(th.mood(20.1), Mood::Gloomy);
        assert_eq!(th.scaled(Pace::SlowBurn).mood(20.0), Mood::Peckish);
        assert_eq!(th.scaled(Pace::Sprint).mood(2.0), Mood::Content);
    }

    #[test]
    fn thresholds_must_increase() {
        assert!(Thresholds::new([1.0, 2.0, 3.0, 4.0]).is_some());
        assert!(Thresholds::new([1.0, 1.0, 3.0, 4.0]).is_none());
        assert!(Thresholds::new([0.0, 1.0, 3.0, 4.0]).is_none());
        assert!(Thresholds::new([1.0, 2.0, f64::NAN, 4.0]).is_none());
    }

    #[test]
    fn plain_span() {
        let d = idle_days(t("2026-10-01T00:00:00Z"), t("2026-10-04T12:00:00Z"), &[], false, &Utc);
        assert_eq!(d, 3.5);
    }

    #[test]
    fn clock_backwards_is_zero() {
        assert_eq!(idle_days(t("2026-10-04T00:00:00Z"), t("2026-10-01T00:00:00Z"), &[], false, &Utc), 0.0);
    }

    #[test]
    fn frozen_and_open_intervals() {
        let last = t("2026-10-01T00:00:00Z");
        let eval = t("2026-10-11T00:00:00Z");
        let holiday = Interval { from: t("2026-10-02T00:00:00Z"), to: Some(t("2026-10-05T00:00:00Z")) };
        let pause = Interval { from: t("2026-10-09T00:00:00Z"), to: None };
        // overlapping with the holiday and before last activity: counted once, clipped
        let early = Interval { from: t("2026-09-20T00:00:00Z"), to: Some(t("2026-10-03T00:00:00Z")) };
        assert_eq!(idle_days(last, eval, &[holiday, pause, early], false, &Utc), 10.0 - 4.0 - 2.0);
    }

    #[test]
    fn weekends_excluded_in_local_time() {
        // Fri 2026-10-02 12:00 local (+05:00) to Tue 2026-10-06 12:00 local = 4 days, minus Sat+Sun
        let tz = FixedOffset::east_opt(5 * 3600).unwrap();
        let last = t("2026-10-02T07:00:00Z");
        let eval = t("2026-10-06T07:00:00Z");
        assert_eq!(idle_days(last, eval, &[], true, &tz), 2.0);
        // starting on a Sunday morning counts none of that Sunday
        let sun = t("2026-10-04T04:00:00Z"); // 09:00 local Sunday
        assert_eq!(idle_days(sun, t("2026-10-05T07:00:00Z"), &[], true, &tz), 0.5);
    }

    fn arb_time() -> impl Strategy<Value = Time> {
        (0i64..400 * 86_400).prop_map(|s| t("2026-01-01T00:00:00Z") + Duration::seconds(s))
    }
    fn arb_frozen() -> impl Strategy<Value = Vec<Interval>> {
        prop::collection::vec(
            (arb_time(), prop::option::of(0i64..30 * 86_400)).prop_map(|(from, len)| Interval {
                from,
                to: len.map(|l| from + Duration::seconds(l)),
            }),
            0..5,
        )
    }

    proptest! {
        #[test]
        fn never_negative_nor_above_span(last in arb_time(), eval in arb_time(), frozen in arb_frozen(), wk: bool) {
            let d = idle_days(last, eval, &frozen, wk, &Utc);
            prop_assert!(d >= 0.0);
            let span = ((eval - last).num_seconds().max(0) as f64) / 86_400.0;
            prop_assert!(d <= span + 1e-9);
        }

        #[test]
        fn later_eval_never_lowers_idle(last in arb_time(), eval in arb_time(), extra in 0i64..30 * 86_400, frozen in arb_frozen(), wk: bool) {
            // A failed sync keeps eval_time; a later successful one can only move it forward.
            let a = idle_days(last, eval, &frozen, wk, &Utc);
            let b = idle_days(last, eval + Duration::seconds(extra), &frozen, wk, &Utc);
            prop_assert!(b + 1e-9 >= a);
        }

        #[test]
        fn freezing_never_raises_idle(last in arb_time(), eval in arb_time(), frozen in arb_frozen(), more in arb_frozen(), wk: bool) {
            let a = idle_days(last, eval, &frozen, wk, &Utc);
            let all: Vec<_> = frozen.iter().chain(&more).copied().collect();
            prop_assert!(idle_days(last, eval, &all, wk, &Utc) <= a + 1e-9);
            prop_assert!(idle_days(last, eval, &frozen, true, &Utc) <= idle_days(last, eval, &frozen, false, &Utc) + 1e-9);
        }

        #[test]
        fn newer_activity_never_raises_idle(last in arb_time(), later in 0i64..30 * 86_400, eval in arb_time(), frozen in arb_frozen(), wk: bool) {
            let a = idle_days(last, eval, &frozen, wk, &Utc);
            prop_assert!(idle_days(last + Duration::seconds(later), eval, &frozen, wk, &Utc) <= a + 1e-9);
        }
    }

}
