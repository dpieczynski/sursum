// SPDX-License-Identifier: GPL-3.0-or-later
//! Installing an application update and its **trial** (ADR 0013, second amendment): a new
//! release must prove itself before it stays, and goes back by itself if it does not.
//!
//! 1. [`install`] verifies a bundle, backs up the database, makes the release `current` (see
//!    [`crate::bundle`]) and writes `trial.json`.
//! 2. Before every start of the service, systemd runs [`guard`] with the **previous** (known
//!    good) binary, never the one on trial: it counts the starts and, after [`MAX_STARTS`]
//!    without a commit, switches back to the previous release, restores the database copy if the
//!    new release had already changed its structure, and removes the failed release.
//! 3. The new server [`commit`]s itself once it has run healthily for [`HEALTHY_AFTER`].
//!
//! A crash, a hang caught by the watchdog, a start timeout or a power cut during the trial all
//! count as starts. The outcome is kept in `update.json` for the remote's "about" section.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_protocol::{InstallRefusal, LastUpdate, UpdateStatus};

use crate::bundle;
use crate::db::Database;

/// The trial of the release that was installed last.
const TRIAL_FILE: &str = "trial.json";
/// How the most recent update ended.
const OUTCOME_FILE: &str = "update.json";
/// Failed releases' databases are kept here after a rollback, never deleted.
#[cfg(unix)]
const ROLLED_BACK_DIR: &str = "rolled-back";
/// Starts a release gets before the guard gives up on it.
pub const MAX_STARTS: u32 = 3;
/// How long a release on trial must run healthily before it stays.
pub const HEALTHY_AFTER: Duration = Duration::from_secs(30);

/// What the HTTP layer needs to take an update from the remote.
#[derive(Clone)]
pub struct Updates {
    /// The key bundles must be signed with; `None` in builds without `SURSUM_BUNDLE_KEY`.
    pub key: Option<ed25519_dalek::VerifyingKey>,
    /// Whether a service manager starts the server again after it exits for the update.
    pub supervised: bool,
    /// Notified once an update is installed: the server should exit so it restarts into it.
    pub restart: std::sync::Arc<tokio::sync::Notify>,
    /// One install at a time.
    pub installing: std::sync::Arc<tokio::sync::Mutex<()>>,
    /// System updates (ADR 0019); off unless the device runs the sursum image.
    pub system: crate::system_update::SystemUpdates,
}

impl Updates {
    /// Whether an upload could be installed at all.
    pub fn can_install(&self) -> bool {
        self.key.is_some() && self.supervised
    }
}

/// `trial.json`. Read by the previous release's guard, so fields are only ever added (with
/// defaults), never renamed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Trial {
    /// The release on trial, relative to the data directory (`releases/<version>`).
    release: PathBuf,
    /// The release to go back to (`previous` at install time); `None` on a first install.
    previous: Option<PathBuf>,
    /// The database copy taken just before the install.
    backup: Option<PathBuf>,
    /// The database schema before the new release first ran.
    schema_before: i64,
    /// Starts counted by the guard so far.
    #[serde(default)]
    starts: u32,
}

/// Why [`install`] changed nothing.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("not a valid bundle for this device: {0:#}")]
    BadBundle(anyhow::Error),
    #[error("version {0} already runs")]
    AlreadyCurrent(String),
    #[error("installing failed: {0:#}")]
    Failed(anyhow::Error),
}

impl InstallError {
    /// The reason as the remote shows it.
    pub fn refusal(&self) -> InstallRefusal {
        match self {
            InstallError::BadBundle(_) => InstallRefusal::BadBundle,
            InstallError::AlreadyCurrent(_) => InstallRefusal::AlreadyCurrent,
            InstallError::Failed(_) => InstallRefusal::Failed,
        }
    }
}

/// What [`guard`] did before a start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guarded {
    /// No release is on trial.
    NoTrial,
    /// The release on trial gets another start (1-based count).
    Start(u32),
    /// The release kept failing; `current` is the previous release again.
    RolledBack(LastUpdate),
    /// The release kept failing but there is nothing to go back to (first install).
    NothingToRestore,
}

/// Verifies `bytes` against `trusted`, backs up the database, installs the release as `current`
/// and puts it on trial. Returns the installed version. Safe while the server runs: nothing
/// touches the database except a plain backup.
pub fn install(
    data_dir: &Path,
    bytes: &[u8],
    trusted: &ed25519_dalek::VerifyingKey,
) -> Result<String, InstallError> {
    let verified = bundle::verify(bytes, trusted).map_err(InstallError::BadBundle)?;
    let version = verified.manifest.version.clone();
    let release = PathBuf::from("releases").join(&version);
    if std::fs::read_link(data_dir.join("current")).ok().as_deref() == Some(release.as_path()) {
        return Err(InstallError::AlreadyCurrent(version));
    }
    let installed = (|| -> anyhow::Result<()> {
        let backup = Database::backup_file(data_dir, &format!("before-{version}"))?;
        let schema = Database::schema_version(data_dir)?;
        bundle::install(&verified, data_dir, schema)?;
        let trial = Trial {
            previous: std::fs::read_link(data_dir.join("previous")).ok(),
            release,
            backup: backup.and_then(|path| path.strip_prefix(data_dir).ok().map(Path::to_path_buf)),
            schema_before: schema,
            starts: 0,
        };
        write_json(data_dir, TRIAL_FILE, &trial)
    })();
    installed.map_err(InstallError::Failed)?;
    Ok(version)
}

/// Runs before each start of the service (with the previous release's binary): counts the
/// start, or rolls back after [`MAX_STARTS`] starts without a commit.
#[cfg(unix)]
pub fn guard(data_dir: &Path) -> anyhow::Result<Guarded> {
    let Some(mut trial) = read_json::<Trial>(data_dir, TRIAL_FILE)? else {
        return Ok(Guarded::NoTrial);
    };
    if std::fs::read_link(data_dir.join("current")).ok().as_ref() != Some(&trial.release) {
        // Someone switched releases by hand; this trial no longer applies.
        remove(data_dir, TRIAL_FILE)?;
        return Ok(Guarded::NoTrial);
    }
    if trial.starts < MAX_STARTS {
        trial.starts += 1;
        write_json(data_dir, TRIAL_FILE, &trial)?;
        return Ok(Guarded::Start(trial.starts));
    }
    let Some(previous) = trial.previous.clone().filter(|previous| data_dir.join(previous).is_dir()) else {
        remove(data_dir, TRIAL_FILE)?;
        return Ok(Guarded::NothingToRestore);
    };
    let database_restored = restore_database_if_migrated(data_dir, &trial)?;
    bundle::switch_current(data_dir, &previous)?;
    // The failed release never runs again, not even as the next guard.
    remove(data_dir, "previous")?;
    std::fs::remove_dir_all(data_dir.join(&trial.release))
        .with_context(|| format!("removing {}", trial.release.display()))?;
    let outcome = LastUpdate::RolledBack {
        failed: version_of(&trial.release),
        restored: version_of(&previous),
        database_restored,
    };
    write_json(data_dir, OUTCOME_FILE, &outcome)?;
    remove(data_dir, TRIAL_FILE)?;
    Ok(Guarded::RolledBack(outcome))
}

/// Ends the trial if `own_release` (the running binary's release folder) is the one on trial.
/// Called by the server after it has run healthily for [`HEALTHY_AFTER`]. Returns the committed
/// version, if any.
pub fn commit(data_dir: &Path, own_release: &Path) -> anyhow::Result<Option<String>> {
    let Some(trial) = read_json::<Trial>(data_dir, TRIAL_FILE)? else {
        return Ok(None);
    };
    let on_trial = std::fs::canonicalize(data_dir.join(&trial.release)).ok();
    if on_trial.is_none() || on_trial != std::fs::canonicalize(own_release).ok() {
        // An older server that has not restarted yet must not vouch for the new release.
        return Ok(None);
    }
    let version = version_of(&trial.release);
    write_json(data_dir, OUTCOME_FILE, &LastUpdate::Committed { version: version.clone() })?;
    remove(data_dir, TRIAL_FILE)?;
    Ok(Some(version))
}

/// The update state for the remote. `can_install` comes from the caller (key and supervision),
/// `system` from [`crate::system_update::SystemUpdates::status`].
pub fn status(
    data_dir: &Path,
    build: &str,
    can_install: bool,
    system: Option<sursum_protocol::SystemStatus>,
) -> UpdateStatus {
    let release = std::fs::read_link(data_dir.join("current")).ok().map(|path| version_of(&path));
    let on_trial = read_json::<Trial>(data_dir, TRIAL_FILE)
        .ok()
        .flatten()
        .is_some_and(|trial| release.as_deref() == Some(version_of(&trial.release).as_str()));
    let last = read_json::<LastUpdate>(data_dir, OUTCOME_FILE).ok().flatten();
    UpdateStatus { build: build.to_owned(), release, on_trial, last, can_install, system }
}

/// If the failed release had already migrated the database, the previous release could not read
/// it: move it aside (kept in `rolled-back/`) and put the pre-update copy back.
#[cfg(unix)]
fn restore_database_if_migrated(data_dir: &Path, trial: &Trial) -> anyhow::Result<bool> {
    if Database::schema_version(data_dir)? <= trial.schema_before {
        return Ok(false);
    }
    let backup = trial
        .backup
        .as_ref()
        .map(|backup| data_dir.join(backup))
        .filter(|backup| backup.is_file())
        .context("the new release changed the database and its pre-update copy is gone")?;
    let aside = data_dir.join(ROLLED_BACK_DIR);
    std::fs::create_dir_all(&aside)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for suffix in ["", "-wal", "-shm"] {
        let file = data_dir.join(format!("{}{suffix}", crate::db::DATABASE_FILE));
        if file.exists() {
            std::fs::rename(&file, aside.join(format!("{stamp}-{}{suffix}", crate::db::DATABASE_FILE)))?;
        }
    }
    let staged = data_dir.join(format!(".{}.restoring", crate::db::DATABASE_FILE));
    std::fs::copy(&backup, &staged)?;
    std::fs::File::open(&staged)?.sync_all()?;
    std::fs::rename(&staged, data_dir.join(crate::db::DATABASE_FILE))?;
    std::fs::File::open(data_dir)?.sync_all()?;
    Ok(true)
}

/// `releases/0.2.0` → `0.2.0`.
fn version_of(release: &Path) -> String {
    release.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn read_json<T: serde::de::DeserializeOwned>(data_dir: &Path, name: &str) -> anyhow::Result<Option<T>> {
    match std::fs::read(data_dir.join(name)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| format!("reading {name}"))?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {name}")),
    }
}

#[cfg(unix)]
fn write_json<T: Serialize>(data_dir: &Path, name: &str, value: &T) -> anyhow::Result<()> {
    bundle::write_atomically(data_dir, name, &serde_json::to_vec_pretty(value)?, 0o644)
}

#[cfg(not(unix))]
fn write_json<T: Serialize>(data_dir: &Path, name: &str, value: &T) -> anyhow::Result<()> {
    Ok(std::fs::write(data_dir.join(name), serde_json::to_vec_pretty(value)?)?)
}

fn remove(data_dir: &Path, name: &str) -> anyhow::Result<()> {
    match std::fs::remove_file(data_dir.join(name)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    struct Device {
        dir: tempfile::TempDir,
        key: ed25519_dalek::SigningKey,
    }

    impl Device {
        fn new() -> Self {
            let key = bundle::signing_key(&bundle::new_key().unwrap().0).unwrap();
            Device { dir: tempfile::tempdir().unwrap(), key }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn install(&self, version: &str) -> Result<String, InstallError> {
            let bytes = bundle::create(version.as_bytes(), version, "b", &self.key).unwrap();
            install(self.path(), &bytes, &self.key.verifying_key())
        }

        fn current(&self) -> PathBuf {
            std::fs::read_link(self.path().join("current")).unwrap()
        }

        /// The release folder as the running binary would see it.
        fn running(&self, version: &str) -> PathBuf {
            self.path().join("releases").join(version)
        }
    }

    #[test]
    fn a_release_that_runs_healthily_commits_itself() {
        let device = Device::new();
        device.install("0.1.0").unwrap();
        device.install("0.2.0").unwrap();
        assert!(status(device.path(), "x", true, None).on_trial);
        assert_eq!(guard(device.path()).unwrap(), Guarded::Start(1));
        // The old server (still running until the restart) cannot commit the new release.
        assert_eq!(commit(device.path(), &device.running("0.1.0")).unwrap(), None);
        assert_eq!(commit(device.path(), &device.running("0.2.0")).unwrap().as_deref(), Some("0.2.0"));
        let status = status(device.path(), "x", true, None);
        assert!(!status.on_trial);
        assert_eq!(status.release.as_deref(), Some("0.2.0"));
        assert_eq!(status.last, Some(LastUpdate::Committed { version: "0.2.0".into() }));
        assert_eq!(guard(device.path()).unwrap(), Guarded::NoTrial);
    }

    #[test]
    fn a_release_that_keeps_failing_goes_back_by_itself() {
        let device = Device::new();
        device.install("0.1.0").unwrap();
        commit(device.path(), &device.running("0.1.0")).unwrap();
        device.install("0.2.0").unwrap();
        for start in 1..=MAX_STARTS {
            assert_eq!(guard(device.path()).unwrap(), Guarded::Start(start));
        }
        let rolled_back = LastUpdate::RolledBack {
            failed: "0.2.0".into(),
            restored: "0.1.0".into(),
            database_restored: false,
        };
        assert_eq!(guard(device.path()).unwrap(), Guarded::RolledBack(rolled_back.clone()));
        assert_eq!(device.current(), Path::new("releases/0.1.0"));
        assert!(!device.running("0.2.0").exists(), "the failed release is removed");
        assert!(std::fs::read_link(device.path().join("previous")).is_err());
        assert_eq!(status(device.path(), "x", true, None).last, Some(rolled_back));
        assert_eq!(guard(device.path()).unwrap(), Guarded::NoTrial);
    }

    #[test]
    fn a_rollback_restores_the_database_the_failed_release_migrated() {
        let device = Device::new();
        let database = Database::open(device.path()).unwrap();
        database.set_setting("marker", &"before").unwrap();
        drop(database);
        device.install("0.1.0").unwrap();
        commit(device.path(), &device.running("0.1.0")).unwrap();
        device.install("0.2.0").unwrap();
        // 0.2.0 starts, changes data and migrates the schema, then keeps crashing.
        let database = Database::open(device.path()).unwrap();
        database.set_setting("marker", &"during the trial").unwrap();
        drop(database);
        let schema = Database::supported_schema();
        rusqlite::Connection::open(device.path().join(crate::db::DATABASE_FILE))
            .unwrap()
            .pragma_update(None, "user_version", schema + 1)
            .unwrap();
        for _ in 0..MAX_STARTS {
            guard(device.path()).unwrap();
        }
        let Guarded::RolledBack(LastUpdate::RolledBack { database_restored, .. }) =
            guard(device.path()).unwrap()
        else {
            panic!("expected a rollback");
        };
        assert!(database_restored);
        assert_eq!(Database::schema_version(device.path()).unwrap(), schema);
        let database = Database::open(device.path()).unwrap();
        assert_eq!(database.setting::<String>("marker").unwrap().as_deref(), Some("before"));
        let kept = std::fs::read_dir(device.path().join(ROLLED_BACK_DIR))
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().ends_with("-sursum.db"))
            .count();
        assert_eq!(kept, 1, "the migrated database is kept aside");
    }

    #[test]
    fn the_running_version_and_strangers_are_refused() {
        let device = Device::new();
        device.install("0.1.0").unwrap();
        assert!(matches!(device.install("0.1.0"), Err(InstallError::AlreadyCurrent(_))));
        let stranger = bundle::signing_key(&bundle::new_key().unwrap().0).unwrap();
        let foreign = bundle::create(b"x", "0.9.0", "b", &stranger).unwrap();
        let refused = install(device.path(), &foreign, &device.key.verifying_key()).unwrap_err();
        assert_eq!(refused.refusal(), InstallRefusal::BadBundle);
        assert_eq!(device.current(), Path::new("releases/0.1.0"));
    }

    #[test]
    fn a_first_install_has_nothing_to_go_back_to() {
        let device = Device::new();
        device.install("0.1.0").unwrap();
        for _ in 0..MAX_STARTS {
            guard(device.path()).unwrap();
        }
        assert_eq!(guard(device.path()).unwrap(), Guarded::NothingToRestore);
        assert_eq!(device.current(), Path::new("releases/0.1.0"));
    }

    #[test]
    fn a_release_switched_by_hand_ends_the_trial() {
        let device = Device::new();
        device.install("0.1.0").unwrap();
        commit(device.path(), &device.running("0.1.0")).unwrap();
        device.install("0.2.0").unwrap();
        bundle::rollback(device.path(), Database::schema_version(device.path()).unwrap()).unwrap();
        assert_eq!(guard(device.path()).unwrap(), Guarded::NoTrial);
        assert!(!status(device.path(), "x", true, None).on_trial);
    }
}
