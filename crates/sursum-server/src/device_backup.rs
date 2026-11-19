// SPDX-License-Identifier: GPL-3.0-or-later
//! Whole-device backups (ADR 0018): a `.sursum-backup` file holds the database (songs with their
//! history, sets, settings, calibration, the program position) and every stored picture, so a
//! device can be brought back after a broken SD card or a mistake.
//!
//! Restoring replaces everything, which the running state actor cannot do under its own feet.
//! An upload is therefore only checked and **staged** in `restore/`, and the server restarts; the
//! next start moves the staged database into place before opening it ([`apply_staged`]). The
//! device keeps its own PIN and paired remotes: a backup never carries them (they are removed on
//! export) and restoring never replaces them, so the remote that restored stays paired.
//!
//! Format: a ZIP with `manifest.json` ([`Manifest`]), `sursum.db` and `assets/<id>` per picture.

use std::io::{Read as _, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_protocol::{BackupSummary, RestoreRefusal};

use crate::db::{DATABASE_FILE, Database, Unreadable};

/// Version of the backup format.
const FORMAT_VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";
const ASSETS_PREFIX: &str = "assets/";
/// Where a checked backup waits for the next start.
const STAGED_DIR: &str = "restore";
/// Prefix of temporary files in the data directory (removed at the next start if left behind).
const TEMPORARY_PREFIX: &str = ".device-backup-";
/// Largest database a backup may unpack to. A parish library is a few megabytes.
const MAX_DATABASE_BYTES: u64 = 1024 * 1024 * 1024;
/// Largest backup accepted for restoring: all pictures this device may hold plus the database.
pub const MAX_BACKUP_BYTES: u64 = crate::assets::MAX_TOTAL_BYTES + MAX_DATABASE_BYTES;

/// `manifest.json`.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    created_unix: u64,
    build: String,
}

/// Why a restore was refused.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("not a sursum backup, or damaged: {0}")]
    NotABackup(String),
    #[error("the backup was made by a newer version of the program")]
    NewerVersion,
    #[error("the backup was made by an earlier version of the program, whose data is not converted")]
    OlderVersion,
    #[error("the backup is too large")]
    TooLarge,
    #[error("could not stage the backup: {0:#}")]
    Failed(anyhow::Error),
}

impl RestoreError {
    /// The reason the remote is told.
    pub fn refusal(&self) -> RestoreRefusal {
        match self {
            RestoreError::NotABackup(_) => RestoreRefusal::NotABackup,
            RestoreError::NewerVersion => RestoreRefusal::NewerVersion,
            RestoreError::OlderVersion => RestoreRefusal::OlderVersion,
            RestoreError::TooLarge => RestoreRefusal::TooLarge,
            RestoreError::Failed(_) => RestoreRefusal::Failed,
        }
    }
}

/// A file in the data directory that is removed when dropped (and at the next start, should the
/// process die first).
pub struct TemporaryFile {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl TemporaryFile {
    /// Creates an empty temporary file in `data_dir`.
    pub fn new(data_dir: &Path) -> std::io::Result<Self> {
        let path = data_dir.join(format!("{TEMPORARY_PREFIX}{}", uuid::Uuid::new_v4()));
        let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
        Ok(TemporaryFile { path, file: Some(file) })
    }

    /// The open file.
    pub fn file(&mut self) -> &mut std::fs::File {
        self.file.as_mut().expect("the file is open until dropped")
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // Close first: Windows cannot remove an open file.
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Writes a backup of the device in `data_dir` to `out` and says what it holds. Safe while the
/// server runs: the database is copied through its own connection.
pub fn create(data_dir: &Path, out: impl Write + Seek) -> anyhow::Result<BackupSummary> {
    let database = TemporaryFile::new(data_dir).context("creating a temporary file")?;
    // `VACUUM INTO` wants to create the file itself.
    std::fs::remove_file(database.path())?;
    let contents =
        Database::copy_without_credentials(data_dir, database.path()).context("copying the database")?;
    let pictures = crate::assets::list(data_dir).context("listing the pictures")?;
    let created_unix =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let manifest = Manifest { format: FORMAT_VERSION, created_unix, build: crate::BUILD_ID.to_owned() };

    let mut zip = zip::ZipWriter::new(out);
    let deflated =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // Pictures are compressed already.
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    zip.start_file(MANIFEST, deflated)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    zip.start_file(DATABASE_FILE, deflated.large_file(true))?;
    std::io::copy(&mut std::fs::File::open(database.path())?, &mut zip)?;
    for (id, path) in &pictures {
        zip.start_file(format!("{ASSETS_PREFIX}{id}"), stored)?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut zip)?;
    }
    zip.finish()?;
    Ok(BackupSummary {
        created_unix,
        build: manifest.build,
        songs: contents.songs,
        sets: contents.sets,
        pictures: u32::try_from(pictures.len()).unwrap_or(u32::MAX),
    })
}

/// Checks the backup in `upload` and stages it for the next start. Pictures are added to the
/// store right away (they are kept by content hash, so this changes nothing that is shown);
/// everything else waits for the restart.
pub fn stage(data_dir: &Path, upload: &std::fs::File) -> Result<BackupSummary, RestoreError> {
    let not_a_backup = |error: &dyn std::fmt::Display| RestoreError::NotABackup(error.to_string());
    let failed = |error: std::io::Error| RestoreError::Failed(error.into());
    let mut archive = zip::ZipArchive::new(upload).map_err(|error| not_a_backup(&error))?;
    let manifest = archive.by_name(MANIFEST).map_err(|error| not_a_backup(&error))?;
    let manifest =
        crate::limits::read_capped(manifest, 64 * 1024, MANIFEST).map_err(|error| not_a_backup(&error))?;
    let manifest: Manifest = serde_json::from_slice(&manifest).map_err(|error| not_a_backup(&error))?;
    if manifest.format > FORMAT_VERSION {
        return Err(RestoreError::NewerVersion);
    }

    let staged = data_dir.join(STAGED_DIR);
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(failed)?;
    }
    std::fs::create_dir_all(&staged).map_err(failed)?;
    let database = staged.join(format!("{DATABASE_FILE}.part"));
    let unpacked = (|| -> Result<(), RestoreError> {
        let entry = archive.by_name(DATABASE_FILE).map_err(|error| not_a_backup(&error))?;
        let mut file = std::fs::File::create(&database).map_err(failed)?;
        let copied = std::io::copy(&mut entry.take(MAX_DATABASE_BYTES + 1), &mut file).map_err(failed)?;
        if copied > MAX_DATABASE_BYTES {
            return Err(RestoreError::TooLarge);
        }
        file.sync_all().map_err(failed)
    })();
    let checked = unpacked.and_then(|()| {
        Database::inspect(&database).map_err(|error| match error {
            Unreadable::Newer => RestoreError::NewerVersion,
            Unreadable::Older => RestoreError::OlderVersion,
            Unreadable::Damaged => not_a_backup(&error),
        })
    });
    let contents = match checked {
        Ok(contents) => contents,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staged);
            return Err(error);
        }
    };

    let pictures = match restore_pictures(data_dir, &mut archive) {
        Ok(pictures) => pictures,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staged);
            return Err(error);
        }
    };
    std::fs::rename(&database, staged.join(DATABASE_FILE)).map_err(failed)?;
    sync_dir(&staged).map_err(failed)?;
    Ok(BackupSummary {
        created_unix: manifest.created_unix,
        build: manifest.build,
        songs: contents.songs,
        sets: contents.sets,
        pictures,
    })
}

/// Adds the backup's pictures to the store; a picture whose bytes do not match its name is
/// skipped (the store names pictures by their hash).
fn restore_pictures(
    data_dir: &Path,
    archive: &mut zip::ZipArchive<&std::fs::File>,
) -> Result<u32, RestoreError> {
    let mut restored = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| RestoreError::NotABackup(error.to_string()))?;
        let Ok(name) = entry.name() else { continue };
        let Some(id) = name.strip_prefix(ASSETS_PREFIX).map(str::to_owned) else { continue };
        let limit = crate::assets::MAX_ASSET_BYTES as u64;
        let Ok(bytes) = crate::limits::read_capped(entry, limit, "a picture") else {
            tracing::warn!(%id, "skipping a picture larger than any upload");
            continue;
        };
        match crate::assets::store(data_dir, &bytes) {
            Ok(stored) if stored.as_str() == id => restored += 1,
            Ok(_) | Err(crate::assets::AssetError::UnsupportedType | crate::assets::AssetError::TooLarge) => {
                tracing::warn!(%id, "skipping a picture that does not match its name");
            }
            Err(error) => return Err(RestoreError::Failed(error.into())),
        }
    }
    Ok(restored)
}

/// At startup, before the database is opened: moves a staged backup into place, keeping this
/// device's PIN and paired remotes, after a copy of the replaced database went to `backups/`.
/// Returns whether a backup was restored. A staged backup that cannot be applied is removed,
/// so a bad file never keeps the server from starting.
pub fn apply_staged(data_dir: &Path) -> anyhow::Result<bool> {
    remove_temporary_files(data_dir);
    let staged_dir = data_dir.join(STAGED_DIR);
    let staged = staged_dir.join(DATABASE_FILE);
    if !staged.exists() {
        if staged_dir.exists() {
            std::fs::remove_dir_all(&staged_dir)?;
        }
        return Ok(false);
    }
    let applied = (|| -> anyhow::Result<()> {
        Database::backup_file(data_dir, "before-restore").context("backing up the replaced database")?;
        let live = data_dir.join(DATABASE_FILE);
        let mut restored = Database::open(&staged_dir).context("opening the staged backup")?;
        restored.adopt_credentials(&live).context("keeping this device's PIN and remotes")?;
        drop(restored);
        for suffix in ["-wal", "-shm"] {
            remove_if_present(&staged_dir.join(format!("{DATABASE_FILE}{suffix}")))?;
            remove_if_present(&data_dir.join(format!("{DATABASE_FILE}{suffix}")))?;
        }
        std::fs::rename(&staged, &live)?;
        sync_dir(data_dir)?;
        Ok(())
    })();
    std::fs::remove_dir_all(&staged_dir)?;
    applied.map(|()| true)
}

fn remove_temporary_files(data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(data_dir) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(TEMPORARY_PREFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Makes a rename durable (a no-op where directories cannot be opened, i.e. Windows).
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    if cfg!(unix) { std::fs::File::open(dir)?.sync_all() } else { Ok(()) }
}

/// Reads a whole temporary file back from its start (for tests and small callers).
#[cfg(test)]
fn read_back(file: &mut std::fs::File) -> Vec<u8> {
    let mut bytes = Vec::new();
    file.rewind().unwrap();
    file.read_to_end(&mut bytes).unwrap();
    bytes
}

#[cfg(test)]
mod tests {
    use sursum_core::model::{ServiceSet, Song, Verse};
    use sursum_protocol::{SetId, SongId};

    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really a picture, but sniffed as one";

    fn song(title: &str) -> Song {
        Song {
            id: SongId::from(title),
            titles: vec![title.into()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: vec![Verse::simple("v1", &["la la"])],
            extra: Default::default(),
        }
    }

    fn device(dir: &Path, pin: &str, device_token: &str, songs: &[&str]) -> Database {
        let mut database = Database::open(dir).unwrap();
        database.set_setting(crate::auth::PIN_SETTING, &pin).unwrap();
        database.add_device(device_token, "Tablet", device_token).unwrap();
        for title in songs {
            database.save_song(&song(title), crate::db::Base::Current).unwrap();
        }
        database
    }

    fn backup_of(dir: &Path) -> std::fs::File {
        let mut file = tempfile::tempfile().unwrap();
        create(dir, &mut file).unwrap();
        file.rewind().unwrap();
        file
    }

    #[test]
    fn a_backup_holds_songs_sets_and_pictures_but_no_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let mut database = device(dir.path(), "old-pin", "old-token", &["Alfa", "Beta"]);
        database
            .save_set(&ServiceSet {
                id: SetId::from("set-1"),
                name: "Niedziela".into(),
                items: vec![],
                date: None,
                church: None,
            })
            .unwrap();
        drop(database);
        crate::assets::store(dir.path(), PNG).unwrap();

        let mut file = tempfile::tempfile().unwrap();
        let summary = create(dir.path(), &mut file).unwrap();
        assert_eq!((summary.songs, summary.sets, summary.pictures), (2, 1, 1));
        let bytes = read_back(&mut file);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut copy = tempfile::NamedTempFile::new().unwrap();
        std::io::copy(&mut archive.by_name(DATABASE_FILE).unwrap(), &mut copy).unwrap();
        let connection = rusqlite::Connection::open(copy.path()).unwrap();
        let devices: i64 =
            connection.query_row("SELECT count(*) FROM devices", [], |row| row.get(0)).unwrap();
        let pins: i64 = connection
            .query_row("SELECT count(*) FROM settings WHERE key = 'pin'", [], |row| row.get(0))
            .unwrap();
        assert_eq!((devices, pins), (0, 0));
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .all(|entry| { !entry.unwrap().file_name().to_string_lossy().starts_with(TEMPORARY_PREFIX) })
        );
    }

    #[test]
    fn a_restored_backup_replaces_the_data_but_keeps_this_devices_pin_and_remotes() {
        let old = tempfile::tempdir().unwrap();
        drop(device(old.path(), "old-pin", "old-token", &["Alfa", "Beta"]));
        crate::assets::store(old.path(), PNG).unwrap();
        let backup = backup_of(old.path());

        let new = tempfile::tempdir().unwrap();
        drop(device(new.path(), "new-pin", "new-token", &["Gamma"]));
        let summary = stage(new.path(), &backup).unwrap();
        assert_eq!((summary.songs, summary.pictures), (2, 1));
        assert_eq!(crate::assets::list(new.path()).unwrap().len(), 1, "pictures are added at once");
        assert_eq!(
            Database::open(new.path()).unwrap().load_songs().unwrap().len(),
            1,
            "songs wait for the restart"
        );

        assert!(apply_staged(new.path()).unwrap());
        let database = Database::open(new.path()).unwrap();
        let titles: Vec<String> =
            database.load_songs().unwrap().into_iter().map(|song| song.titles[0].clone()).collect();
        assert_eq!(titles, ["Alfa", "Beta"]);
        assert_eq!(database.setting::<String>(crate::auth::PIN_SETTING).unwrap().as_deref(), Some("new-pin"));
        assert_eq!(database.device_token_hashes().unwrap().len(), 1);
        assert!(!new.path().join(STAGED_DIR).exists());
        let replaced = std::fs::read_dir(new.path().join("backups"))
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().ends_with("-before-restore.db"));
        assert!(replaced, "the replaced database is kept in backups/");
        assert!(!apply_staged(new.path()).unwrap(), "nothing is staged any more");
    }

    #[test]
    fn foreign_files_and_newer_backups_are_refused_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        drop(device(dir.path(), "pin", "token", &["Alfa"]));
        let mut junk = tempfile::tempfile().unwrap();
        junk.write_all(b"not a zip").unwrap();
        junk.rewind().unwrap();
        assert!(matches!(stage(dir.path(), &junk), Err(RestoreError::NotABackup(_))));

        let mut newer = tempfile::tempfile().unwrap();
        let mut zip = zip::ZipWriter::new(&mut newer);
        zip.start_file(MANIFEST, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format": 99, "created_unix": 0, "build": "9.9.9"}"#).unwrap();
        zip.finish().unwrap();
        newer.rewind().unwrap();
        assert!(matches!(stage(dir.path(), &newer), Err(RestoreError::NewerVersion)));

        let mut not_sursum = tempfile::tempfile().unwrap();
        let other = tempfile::NamedTempFile::new().unwrap();
        rusqlite::Connection::open(other.path()).unwrap().execute_batch("CREATE TABLE t (x);").unwrap();
        let mut zip = zip::ZipWriter::new(&mut not_sursum);
        zip.start_file(MANIFEST, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format": 1, "created_unix": 0, "build": "x"}"#).unwrap();
        zip.start_file(DATABASE_FILE, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&std::fs::read(other.path()).unwrap()).unwrap();
        zip.finish().unwrap();
        not_sursum.rewind().unwrap();
        assert!(matches!(stage(dir.path(), &not_sursum), Err(RestoreError::NotABackup(_))));

        assert!(!dir.path().join(STAGED_DIR).exists());
        assert!(!apply_staged(dir.path()).unwrap());
        assert_eq!(Database::open(dir.path()).unwrap().load_songs().unwrap().len(), 1);
    }
}
