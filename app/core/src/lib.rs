//! Strays core: all state and decisions, independent of Tauri so it can be tested with a fake clock.
pub mod clock;
pub mod display;
pub mod github;
pub mod localgit;
pub mod mood;
pub mod nudge;
pub mod pet;
pub mod report;
pub mod settings;
pub mod store;

// Re-exported so the app uses the same versions without depending on them separately.
pub use chrono;
pub use uuid;
