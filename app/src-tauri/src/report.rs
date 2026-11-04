//! Error and crash reports, sent only when the user opted in (Settings → Privacy, or the welcome screen).
//! Builds without a PostHog key (`STRAYS_POSTHOG_KEY` at compile time) can't send any and hide the option.
use strays_core::report::Reporter;

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

/// A report for a problem shown to the user, or `None` when it's not an error. `key` is
/// `category:detail`; the report carries only that code (a pet's id becomes `pet`), never the
/// message, so nothing typed, named or found on the Mac can leave in one.
pub fn for_issue(key: &str) -> Option<Report> {
    let kind = key.split(':').next().unwrap_or(key);
    let code: Vec<String> = key.split(':').map(|part| {
        let part = part.strip_prefix("3d-").filter(|rest| rest.parse::<strays_core::uuid::Uuid>().is_ok()).map_or(part, |_| "3d-pet");
        let part = part.strip_prefix("draw-").filter(|rest| rest.parse::<strays_core::uuid::Uuid>().is_ok()).map_or(part, |_| "draw-pet");
        if part.parse::<strays_core::uuid::Uuid>().is_ok() { "pet".into() } else { part.chars().take(40).collect() }
    }).collect();
    (!NOT_ERRORS.contains(&kind)).then(|| Report { kind: kind.into(), message: code.join(":"), handled: true })
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

/// The report for a crash file's text: only where it happened, as a source file name and line (no
/// folders, not even the build machine's), never the panic message.
pub fn for_crash(text: &str) -> Report {
    let at = text.lines().next().unwrap_or_default();
    let at = at.rsplit(['/', '\\']).next().unwrap_or(at);
    Report { kind: "crash".into(), message: format!("at {at}"), handled: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_arent_reported_and_crashes_keep_their_location() {
        assert_eq!(for_issue("missing"), None);
        assert_eq!(for_issue("action"), None);
        // codes only: a pet's id becomes `pet`, and no message text goes at all
        let id = strays_core::uuid::Uuid::new_v4();
        assert_eq!(for_issue(&format!("save:{id}")).unwrap().message, "save:pet");
        assert_eq!(for_issue(&format!("overlay:3d-{id}")).unwrap().message, "overlay:3d-pet");
        assert_eq!(for_issue("overlay:3d-gpu").unwrap().message, "overlay:3d-gpu");
        let r = for_crash("/Users/builder/strays/app/src-tauri/src/state.rs:42\ncalled unwrap on /Users/me/ClientX\n\nbacktrace…");
        assert_eq!((r.kind.as_str(), r.message.as_str(), r.handled), ("crash", "at state.rs:42", false));
    }
}
