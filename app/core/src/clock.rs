use std::cell::Cell;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub type Time = DateTime<Utc>;

/// The only way core code reads the time (spec §11), so tests and `--time-warp` can control it.
pub trait Clock {
    fn now(&self) -> Time;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Time {
        Utc::now()
    }
}

/// Test and time-warp clock: starts at a fixed instant and only moves when told.
pub struct FakeClock(Cell<Time>);

impl FakeClock {
    pub fn at(t: Time) -> Self {
        Self(Cell::new(t))
    }
    pub fn advance(&self, d: Duration) {
        self.0.set(self.0.get() + d);
    }
    pub fn set(&self, t: Time) {
        self.0.set(t);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Time {
        self.0.get()
    }
}

/// A half-open time range. `to: None` means still ongoing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub from: Time,
    pub to: Option<Time>,
}
