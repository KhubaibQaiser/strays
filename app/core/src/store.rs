//! JSON files in the app's data folder, written atomically (spec §11.1):
//! `pets/<uuid>.json`, `settings.json`, `sync-state.json`.
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::pet::{Pet, SCHEMA_VERSION};
use crate::settings::Settings;

pub struct Store {
    root: PathBuf,
    /// Set by `close`: no write reaches the folder afterwards (Delete everything).
    closed: std::sync::atomic::AtomicBool,
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// The file is unreadable JSON or fails validation. It is left untouched for the user or a later version.
    Corrupt { path: PathBuf, why: String },
    /// Written by a newer Strays; never overwritten by this one.
    TooNew { path: PathBuf, version: u32 },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "Couldn't read or write the data folder: {e}"),
            StoreError::Corrupt { path, .. } => write!(f, "{} can't be read; it was left untouched", path.display()),
            StoreError::TooNew { path, .. } => write!(f, "{} was written by a newer Strays; update Strays to use it", path.display()),
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        fs::create_dir_all(root.join("pets"))?;
        // a crash between writing a temp file and renaming it leaves the temp file behind
        for dir in [root.clone(), root.join("pets")] {
            for e in fs::read_dir(&dir)?.flatten() {
                if e.file_name().to_string_lossy().contains(".tmp-") {
                    fs::remove_file(e.path())?;
                }
            }
        }
        Ok(Self { root, closed: Default::default() })
    }

    /// From now on every save is skipped, so a folder being deleted can't be recreated by a late write.
    pub fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn write<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), StoreError> {
        if self.is_closed() {
            return Ok(());
        }
        write_atomic(path, value)
    }

    /// Moves an unreadable file to `<name>.corrupt` (replacing an older copy, so copies can't pile up) and
    /// returns where it went. The caller then starts fresh without destroying what was there.
    pub fn set_aside(&self, name: &str) -> Result<PathBuf, StoreError> {
        let kept = self.root.join(format!("{name}.corrupt"));
        fs::rename(self.root.join(name), &kept)?;
        Ok(kept)
    }

    fn pet_path(&self, id: Uuid) -> PathBuf {
        self.root.join("pets").join(format!("{id}.json"))
    }

    /// Loads every pet. A bad file is reported and skipped, never deleted, so one corrupt pet can't hide the rest.
    pub fn load_pets(&self) -> Result<(Vec<Pet>, Vec<StoreError>), StoreError> {
        let mut pets = Vec::new();
        let mut errors = Vec::new();
        for entry in fs::read_dir(self.root.join("pets"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                match read_json::<Pet>(&path).and_then(|p| check_version(&path, p.schema_version).map(|_| p)) {
                    Ok(p) => pets.push(p),
                    Err(e) => errors.push(e),
                }
            }
        }
        pets.sort_by_key(|p| p.created_at);
        Ok((pets, errors))
    }

    pub fn save_pet(&self, pet: &Pet) -> Result<(), StoreError> {
        self.write(&self.pet_path(pet.id), pet)
    }

    /// Missing file → defaults. A corrupt or invalid file is an error, so the caller can tell the user
    /// rather than silently resetting their settings.
    pub fn load_settings(&self) -> Result<Settings, StoreError> {
        let path = self.root.join("settings.json");
        if !path.exists() {
            return Ok(Settings::default());
        }
        let s: Settings = read_json(&path)?;
        s.validate().map_err(|why| StoreError::Corrupt { path, why })?;
        Ok(s)
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), StoreError> {
        s.validate().map_err(|why| StoreError::Corrupt { path: self.root.join("settings.json"), why })?;
        self.write(&self.root.join("settings.json"), s)
    }

    pub fn load<T: DeserializeOwned + Default>(&self, name: &str) -> Result<T, StoreError> {
        let path = self.root.join(name);
        if path.exists() { read_json(&path) } else { Ok(T::default()) }
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> Result<(), StoreError> {
        self.write(&self.root.join(name), value)
    }
}

fn check_version(path: &Path, version: u32) -> Result<(), StoreError> {
    if version > SCHEMA_VERSION {
        return Err(StoreError::TooNew { path: path.into(), version });
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, StoreError> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt { path: path.into(), why: e.to_string() })
}

/// Write to a temp file in the same folder, fsync, then rename over the target, so a crash or
/// power loss leaves either the old file or the new one, never half of each.
fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    let json = serde_json::to_vec_pretty(value).expect("our types always serialise");
    let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        // the rename itself is only durable once the folder is synced too
        fs::File::open(path.parent().unwrap_or(Path::new("."))).and_then(|d| d.sync_all())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mood::Pace;
    use crate::pet::{Activity, ActivitySource, Project, Species};

    fn sample() -> Pet {
        let now = "2026-10-01T00:00:00Z".parse().unwrap();
        Pet::new(now, "Miso", Species::Cat, Project::default(), Activity { last: now, source: ActivitySource::LocalCommit }, Pace::Normal)
    }

    #[test]
    fn pets_round_trip_and_bad_files_are_skipped_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let p = sample();
        store.save_pet(&p).unwrap();
        fs::write(dir.path().join("pets/broken.json"), b"{not json").unwrap();
        let mut future = serde_json::to_value(sample()).unwrap();
        future["schemaVersion"] = 99.into();
        fs::write(dir.path().join("pets/future.json"), future.to_string()).unwrap();

        let (pets, errors) = store.load_pets().unwrap();
        assert_eq!(pets, vec![p]);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|e| matches!(e, StoreError::TooNew { version: 99, .. })));
        assert!(dir.path().join("pets/broken.json").exists());
        // no temp files left behind
        let names: Vec<_> = fs::read_dir(dir.path().join("pets")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
    }

    #[test]
    fn leftover_temp_files_are_removed_and_corrupt_files_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        Store::open(dir.path()).unwrap();
        fs::write(dir.path().join("sync-state.tmp-1234"), b"half").unwrap();
        fs::write(dir.path().join("pets/abc.tmp-5678"), b"half").unwrap();
        fs::write(dir.path().join("sync-state.json"), b"{broken").unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert!(!dir.path().join("sync-state.tmp-1234").exists());
        assert!(!dir.path().join("pets/abc.tmp-5678").exists());
        assert!(matches!(store.load::<crate::github::SyncState>("sync-state.json"), Err(StoreError::Corrupt { .. })));
        let kept = store.set_aside("sync-state.json").unwrap();
        assert_eq!(fs::read(&kept).unwrap(), b"{broken");
        assert_eq!(store.load::<crate::github::SyncState>("sync-state.json").unwrap(), Default::default());
        // a second corrupt file replaces the first copy rather than adding another
        fs::write(dir.path().join("sync-state.json"), b"{again").unwrap();
        store.set_aside("sync-state.json").unwrap();
        assert_eq!(fs::read(&kept).unwrap(), b"{again");
    }

    #[test]
    fn a_closed_store_never_recreates_a_deleted_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data");
        let store = Store::open(&root).unwrap();
        store.close();
        fs::remove_dir_all(&root).unwrap();
        store.save_pet(&sample()).unwrap();
        store.save("sync-state.json", &1).unwrap();
        store.save_settings(&Settings::default()).unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn settings_default_when_missing_error_when_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.load_settings().unwrap(), Settings::default());
        let s = Settings { slots: 7, ..Settings::default() };
        store.save_settings(&s).unwrap();
        assert_eq!(store.load_settings().unwrap().slots, 7);
        assert!(store.save_settings(&Settings { slots: 0, ..Settings::default() }).is_err());
        fs::write(dir.path().join("settings.json"), r#"{"slots": 0}"#).unwrap();
        assert!(matches!(store.load_settings(), Err(StoreError::Corrupt { .. })));
    }
}
