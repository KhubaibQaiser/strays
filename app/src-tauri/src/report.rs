//! Error and crash reports, sent only when the user opted in (Settings → Privacy, or the welcome screen).
//! Builds without a PostHog key (`STRAYS_POSTHOG_KEY` at compile time) can't send any and hide the option.
use strays_core::report::{Reporter, scrub};

pub use strays_core::report::Report;

const KEY: Option<&str> = option_env!("STRAYS_POSTHOG_KEY");
/// PostHog's US ingestion host unless the build names another (an unset CI variable arrives empty).
fn host() -> &'static str {
    option_env!("STRAYS_POSTHOG_HOST").filter(|h| !h.is_empty()).unwrap_or("https://us.i.posthog.com")
}
/// At most this many reports a day leave the Mac, however much goes wrong.
pub const PER_DAY: usize = 20;
/// Kept on file when sending is slow; older ones are dropped first.
pub const QUEUE: usize = 20;
/// Issue categories that are the user's decisions, not errors, so never reported.
const NOT_ERRORS: [&str; 2] = ["missing", "action"];
pub const CRASH_FILE: &str = "crash.txt";

pub fn available() -> bool {
    // tests queue reports without a key; they never send
    cfg!(test) || KEY.is_some_and(|k| !k.is_empty())
}

pub fn reporter() -> Result<Reporter, String> {
    Reporter::new(host(), KEY.filter(|k| !k.is_empty()).ok_or("this build can't send error reports")?)
}

/// A report for a problem shown to the user, or `None` when it's not an error. `key` is `category:detail`.
pub fn for_issue(key: &str, text: &str, private: &[&str]) -> Option<Report> {
    let kind = key.split(':').next().unwrap_or(key);
    (!NOT_ERRORS.contains(&kind)).then(|| Report { kind: kind.into(), message: scrub(text, private), handled: true })
}

/// Saves what a panic said for the next launch to show and report (the app aborts right after this).
pub fn install_crash_hook(dir: std::path::PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
        let text = format!("{at}\n{msg}\n\n{}", std::backtrace::Backtrace::force_capture());
        if let Err(e) = std::fs::write(dir.join(CRASH_FILE), &text) {
            crate::log::warn(&format!("could not save the crash details: {e}"));
        }
        crate::log::write("CRASH", &text);
        default(info);
    }));
}

/// The report for a crash file's text: where it happened (our source, not personal) and the scrubbed message.
pub fn for_crash(text: &str, private: &[&str]) -> Report {
    let mut lines = text.lines();
    let at = lines.next().unwrap_or_default();
    let msg = lines.next().unwrap_or_default();
    Report { kind: "crash".into(), message: format!("{} at {at}", scrub(msg, private)), handled: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_arent_reported_and_crashes_keep_their_location() {
        assert_eq!(for_issue("missing", "GitHub can't find Miso's repo", &["Miso"]), None);
        assert_eq!(for_issue("action", "All slots are full", &[]), None);
        assert_eq!(for_issue("save:1234", "Couldn't save Miso: disk full", &["Miso"]).unwrap().message, "Couldn't save <name>: disk full");
        let r = for_crash("src/state.rs:42\ncalled unwrap on /Users/me/x\n\nbacktrace…", &[]);
        assert_eq!((r.kind.as_str(), r.message.as_str(), r.handled), ("crash", "called unwrap on <path> at src/state.rs:42", false));
    }
}
