//! The app's log file, `logs/strays.log` in the data folder: what went wrong, for bug reports. It's capped
//! at `MAX` bytes; when full it becomes `strays.log.1` (replacing the older one), so it never grows past 2×`MAX`.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX: u64 = 1 << 20;
static FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(data_dir: &Path) -> std::io::Result<()> {
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir)?;
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.join("strays.log"));
    // each launch says what it ran on, so a log sent with a bug report explains itself
    write("START", &format!("Strays {} on macOS {}, {}", env!("CARGO_PKG_VERSION"), crate::platform::os_version(), std::env::consts::ARCH));
    Ok(())
}

/// Appends one timestamped line (and echoes it to stderr for dev runs). Never fails: the log is where failures go.
pub fn write(level: &str, msg: &str) {
    let line = format!("{} {level} {msg}\n", strays_core::chrono::Utc::now().to_rfc3339_opts(strays_core::chrono::SecondsFormat::Secs, true));
    eprint!("{line}");
    let Some(path) = FILE.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
    if let Err(e) = append(&path, &line) {
        eprintln!("could not write the log: {e}");
    }
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX) {
        std::fs::rename(path, path.with_extension("log.1"))?;
    }
    std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())
}

/// Stops writing the log file (Delete everything removes it); lines still go to stderr.
pub fn stop() {
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

pub fn warn(msg: &str) {
    write("WARN", msg);
}

pub fn path() -> Option<PathBuf> {
    FILE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_log_rolls_over_instead_of_growing() {
        // through `append`, as other tests log through the shared file at the same time
        let dir = std::env::temp_dir().join(format!("strays-log-{}", strays_core::uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("strays.log");
        std::fs::write(&path, vec![b'x'; super::MAX as usize + 1]).unwrap();
        super::append(&path, "x WARN after the cap\n").unwrap();
        assert_eq!(std::fs::metadata(path.with_extension("log.1")).unwrap().len(), super::MAX + 1);
        assert!(std::fs::read_to_string(&path).unwrap().ends_with("WARN after the cap\n"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
