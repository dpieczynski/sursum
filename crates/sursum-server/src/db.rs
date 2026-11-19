// SPDX-License-Identifier: GPL-3.0-or-later
//! SQLite persistence. Only the state actor writes, so there is a single connection.
//!
//! Migrations are plain SQL scripts applied in order, tracked with `PRAGMA user_version`. Songs
//! are versioned documents (ADR 0025, the `songs` submodule).
//! Durability settings follow the plan: WAL with `synchronous=FULL` (writes are rare, so an
//! fsync per commit costs nothing and survives power loss).

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sursum_protocol::{ItemId, Overlay, SetId, SlideKey};

mod churches;
mod sets;
mod songs;
mod versions;

pub use churches::{Church, church_name};
pub use sets::{set_body, set_from_body};
pub use songs::{Base, SongImport, SongSave, song_body, song_from_body};

/// Schema versions start above this (`user_version` = this + the number of migrations
/// applied). Databases of earlier builds have 1 to 5 and are refused, never converted: nobody
/// used them in a church (ADR 0025). Old releases, in turn, refuse these as newer.
const SCHEMA_BASE: usize = 100;

/// Schema migrations; `SCHEMA_BASE` + index + 1 is the resulting `user_version`. Never edit a
/// released entry, only append.
const MIGRATIONS: &[&str] = &[
    // 101: runtime state, settings, sets, paired devices, and the versions of songs (ADR 0025).
    "CREATE TABLE runtime_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        json TEXT NOT NULL,
        boot_id TEXT,
        saved_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        json TEXT NOT NULL
    );
    CREATE TABLE sets (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        deleted_at TEXT
    );
    CREATE TABLE set_items (
        set_id TEXT NOT NULL REFERENCES sets(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        item_id TEXT NOT NULL,
        kind_json TEXT NOT NULL,
        PRIMARY KEY (set_id, position)
    );
    -- Paired remote devices (only a hash of each device's token is stored).
    CREATE TABLE devices (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        token_hash TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        revoked_at TEXT
    );
    -- Every version of every document; `head` marks the versions nothing replaces. `body` is
    -- NULL for a deletion; `ancestors` are version ids separated by spaces, sorted.
    CREATE TABLE versions (
        kind TEXT NOT NULL,
        document_id TEXT NOT NULL,
        version_id TEXT NOT NULL,
        body BLOB,
        ancestors TEXT NOT NULL,
        head INTEGER NOT NULL,
        saved_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (kind, document_id, version_id)
    );
    CREATE INDEX versions_heads ON versions (kind, document_id) WHERE head = 1;",
    // 102: the version the last OpenSong import made of each song on this device (ADR 0025): a
    // later import builds on it, so it never undoes a correction made in sursum. Never travels.
    "CREATE TABLE imported_versions (
        song_id TEXT PRIMARY KEY,
        version_id TEXT NOT NULL,
        body BLOB NOT NULL,
        ancestors TEXT NOT NULL
    );",
    // 103: sets are versioned documents too (ADR 0025), kept in `versions`.
    "DROP TABLE set_items;
    DROP TABLE sets;",
];

/// The database file inside the data directory.
pub(crate) const DATABASE_FILE: &str = "sursum.db";
/// Backup copies, newest kept.
const BACKUP_DIR: &str = "backups";
/// How many backup copies to keep.
pub const BACKUPS_KEPT: usize = 14;

/// `PRAGMA application_id` marking the file as a sursum database ("SURS").
const APPLICATION_ID: i64 = 0x5355_5253;

/// The program position saved after every change, restored at startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedProgram {
    pub set_id: Option<SetId>,
    pub item_id: Option<ItemId>,
    pub slide_key: Option<SlideKey>,
    pub overlay: Overlay,
    /// True during the gap after the item's last slide.
    #[serde(default)]
    pub gap: bool,
}

/// A saved program together with the boot it was saved in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRecord {
    pub program: SavedProgram,
    pub boot_id: Option<String>,
}

/// The database connection.
pub struct Database {
    connection: Connection,
}

impl Database {
    /// Opens (or creates) `sursum.db` in `data_dir` and brings the schema up to date.
    ///
    /// If the file is damaged (e.g. after a power cut on a bad SD card), it is moved to
    /// `corrupt/` and the newest backup from `backups/` is restored instead. Only real damage
    /// does that: a schema newer than this program (an older release after a rollback), a busy
    /// file or a permission problem is an error that leaves the file alone.
    pub fn open(data_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_dir).with_context(|| format!("creating {}", data_dir.display()))?;
        let path = data_dir.join(DATABASE_FILE);
        match Self::open_checked(&path) {
            Ok(database) => Ok(database),
            Err(error) if path.exists() && is_damage(&error) => {
                tracing::error!(?error, "the database is damaged; restoring the newest backup");
                restore_newest_backup(data_dir)?;
                Self::open_checked(&path).context("opening the restored backup")
            }
            Err(error) => Err(error),
        }
    }

    /// Opens a database file and verifies its integrity with `PRAGMA quick_check`.
    fn open_checked(path: &Path) -> anyhow::Result<Self> {
        let database = Self::configure(Connection::open(path)?)?;
        let check: String = database.connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(anyhow::Error::new(Damaged(check)));
        }
        Ok(database)
    }

    /// The schema version of the database in `data_dir` (0 if there is none yet), read without
    /// migrating or repairing anything.
    pub fn schema_version(data_dir: &Path) -> anyhow::Result<i64> {
        let path = data_dir.join(DATABASE_FILE);
        if !path.exists() {
            return Ok(0);
        }
        let connection = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// The schema version this program creates and understands.
    pub fn supported_schema() -> i64 {
        i64::try_from(SCHEMA_BASE + MIGRATIONS.len()).unwrap_or(i64::MAX)
    }

    /// A backup of the database in `data_dir` through a plain connection: no migration and no
    /// repair, so it is safe while the server is running (e.g. before installing an update).
    pub fn backup_file(data_dir: &Path, label: &str) -> anyhow::Result<Option<PathBuf>> {
        let path = data_dir.join(DATABASE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let connection = Connection::open(&path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let dir = data_dir.join(BACKUP_DIR);
        std::fs::create_dir_all(&dir)?;
        let seconds =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let target = dir.join(format!("sursum-{seconds:012}-{label}.db"));
        connection.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        prune_backups(&dir)?;
        Ok(Some(target))
    }

    /// Writes a consistent copy of the database into `data_dir/backups` and keeps the newest
    /// [`BACKUPS_KEPT`] copies. Called at startup and before imports.
    pub fn backup(&self, data_dir: &Path, label: &str) -> anyhow::Result<PathBuf> {
        let dir = data_dir.join(BACKUP_DIR);
        std::fs::create_dir_all(&dir)?;
        let seconds =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let target = dir.join(format!("sursum-{seconds:012}-{label}.db"));
        if target.exists() {
            std::fs::remove_file(&target)?;
        }
        self.connection.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        prune_backups(&dir)?;
        Ok(target)
    }

    /// Writes a consistent copy of the database in `data_dir` to `target` for a device backup,
    /// without what lets anyone in (the PIN hash and the paired devices): a backup file may be
    /// carried around on a stick, and restoring keeps the device's own credentials anyway.
    /// Works through a plain connection, so it is safe while the server runs.
    pub(crate) fn copy_without_credentials(data_dir: &Path, target: &Path) -> anyhow::Result<Contents> {
        let live = Connection::open(data_dir.join(DATABASE_FILE))?;
        live.busy_timeout(std::time::Duration::from_secs(5))?;
        live.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        drop(live);
        let copy = Connection::open(target)?;
        copy.execute_batch(&format!(
            "DELETE FROM devices; DELETE FROM settings WHERE key = '{}'; VACUUM;",
            crate::auth::PIN_SETTING
        ))?;
        contents(&copy)
    }

    /// Checks that `path` is a sursum database this program can open (without changing it) and
    /// says what it holds.
    pub(crate) fn inspect(path: &Path) -> Result<Contents, Unreadable> {
        let damaged = |_| Unreadable::Damaged;
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(damaged)?;
        let application: i64 =
            connection.pragma_query_value(None, "application_id", |row| row.get(0)).map_err(damaged)?;
        let version: i64 =
            connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(damaged)?;
        if application != APPLICATION_ID || version < 1 {
            return Err(Unreadable::Damaged);
        }
        if version <= i64::try_from(SCHEMA_BASE).unwrap_or(i64::MAX) {
            return Err(Unreadable::Older);
        }
        if version > Self::supported_schema() {
            return Err(Unreadable::Newer);
        }
        let check: String =
            connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(damaged)?;
        if check != "ok" {
            return Err(Unreadable::Damaged);
        }
        contents(&connection).map_err(|_| Unreadable::Damaged)
    }

    /// Replaces this database's PIN and paired devices with those of the database at `live`
    /// (if there is one), so restoring a backup never locks out the remote that restored it.
    pub(crate) fn adopt_credentials(&mut self, live: &Path) -> anyhow::Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM devices", [])?;
        transaction.execute("DELETE FROM settings WHERE key = ?1", [crate::auth::PIN_SETTING])?;
        transaction.commit()?;
        if !live.exists() {
            return Ok(());
        }
        self.connection.execute("ATTACH DATABASE ?1 AS live", [live.to_string_lossy()])?;
        let copied = self.connection.execute_batch(&format!(
            "BEGIN;
             INSERT INTO main.devices (id, name, token_hash, created_at, revoked_at)
                 SELECT id, name, token_hash, created_at, revoked_at FROM live.devices;
             INSERT INTO main.settings (key, json) SELECT key, json FROM live.settings WHERE key = '{}';
             COMMIT;",
            crate::auth::PIN_SETTING
        ));
        if copied.is_err() {
            let _ = self.connection.execute_batch("ROLLBACK");
        }
        self.connection.execute("DETACH DATABASE live", [])?;
        copied?;
        // Fold the write-ahead log into the file, which is about to be moved into place.
        self.connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    /// An in-memory database, for tests.
    pub fn in_memory() -> anyhow::Result<Self> {
        Self::configure(Connection::open_in_memory()?)
    }

    fn configure(connection: Connection) -> anyhow::Result<Self> {
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "trusted_schema", "OFF")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut database = Database { connection };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&mut self) -> anyhow::Result<()> {
        let version: i64 = self.connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let version = usize::try_from(version).context("negative schema version")?;
        let applied = match version {
            0 => 0,
            1..=SCHEMA_BASE => return Err(EarlierBuild(version).into()),
            _ => version - SCHEMA_BASE,
        };
        if applied > MIGRATIONS.len() {
            anyhow::bail!(
                "database schema {version} is newer than this program ({}); refusing to start",
                SCHEMA_BASE + MIGRATIONS.len()
            );
        }
        for (index, script) in MIGRATIONS.iter().enumerate().skip(applied) {
            let schema = SCHEMA_BASE + index + 1;
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(script).with_context(|| format!("applying migration {schema}"))?;
            transaction.pragma_update(None, "user_version", i64::try_from(schema)?)?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
            transaction.commit()?;
        }
        Ok(())
    }

    /// Records a newly paired device.
    pub fn add_device(&self, id: &str, name: &str, token_hash: &str) -> anyhow::Result<()> {
        self.connection.execute(
            "INSERT INTO devices (id, name, token_hash) VALUES (?1, ?2, ?3)",
            params![id, name, token_hash],
        )?;
        Ok(())
    }

    /// Paired devices that are not revoked: `(id, name, token_hash, paired_at)`, oldest first.
    pub fn devices(&self) -> anyhow::Result<Vec<(String, String, String, String)>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, token_hash, created_at FROM devices WHERE revoked_at IS NULL ORDER BY created_at, id",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Marks a device as revoked; returns its token hash (`None` if there is no such device).
    pub fn revoke_device(&self, id: &str) -> anyhow::Result<Option<String>> {
        let hash: Option<String> = self
            .connection
            .query_row("SELECT token_hash FROM devices WHERE id = ?1 AND revoked_at IS NULL", [id], |row| {
                row.get(0)
            })
            .optional()?;
        if hash.is_some() {
            self.connection.execute("UPDATE devices SET revoked_at = datetime('now') WHERE id = ?1", [id])?;
        }
        Ok(hash)
    }

    /// Token hashes of all devices that are not revoked.
    pub fn device_token_hashes(&self) -> anyhow::Result<Vec<String>> {
        let mut statement =
            self.connection.prepare("SELECT token_hash FROM devices WHERE revoked_at IS NULL")?;
        let hashes = statement.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;
        Ok(hashes)
    }

    /// Reads a JSON setting.
    pub fn setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> anyhow::Result<Option<T>> {
        let json: Option<String> = self
            .connection
            .query_row("SELECT json FROM settings WHERE key = ?1", [key], |row| row.get(0))
            .optional()?;
        json.map(|json| serde_json::from_str(&json).context("decoding a setting")).transpose()
    }

    /// Writes a JSON setting.
    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> anyhow::Result<()> {
        self.connection.execute(
            "INSERT INTO settings (key, json) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET json = excluded.json",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// Runs `work` in one transaction: all of it is stored, or nothing. Bulk changes (merge,
    /// import) also need one disk sync instead of one per song, which matters on an SD card.
    pub fn in_transaction<T>(
        &mut self,
        work: impl FnOnce(&mut Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        match work(self) {
            Ok(value) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    /// Loads the last saved program position.
    pub fn load_runtime(&self) -> anyhow::Result<Option<RuntimeRecord>> {
        let row: Option<(String, Option<String>)> = self
            .connection
            .query_row("SELECT json, boot_id FROM runtime_state WHERE id = 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        let Some((json, boot_id)) = row else {
            return Ok(None);
        };
        let program = serde_json::from_str(&json).context("decoding saved runtime state")?;
        Ok(Some(RuntimeRecord { program, boot_id }))
    }

    /// Saves the program position (one row, replaced on every change).
    pub fn save_runtime(&self, program: &SavedProgram, boot_id: Option<&str>) -> anyhow::Result<()> {
        let json = serde_json::to_string(program)?;
        self.connection.execute(
            "INSERT INTO runtime_state (id, json, boot_id, saved_at) VALUES (1, ?1, ?2, datetime('now'))
             ON CONFLICT (id) DO UPDATE SET json = excluded.json, boot_id = excluded.boot_id, saved_at = excluded.saved_at",
            params![json, boot_id],
        )?;
        Ok(())
    }
}

/// What a database holds, for the summary of a device backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Contents {
    pub songs: u32,
    pub sets: u32,
}

/// Why a database file from a backup cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Unreadable {
    #[error("not a sursum database, or damaged")]
    Damaged,
    #[error("made by a newer version of the program")]
    Newer,
    #[error("made by an earlier version of the program, whose data is not converted")]
    Older,
}

/// A database written by an earlier build of the program (schema 1 to 5). It is not converted
/// (nobody used those builds in a church, ADR 0025) and not mistaken for a damaged file.
#[derive(Debug, thiserror::Error)]
#[error(
    "the database was written by an earlier version of sursum (schema {0}) and is not converted; \
     move the data folder aside to start with an empty one"
)]
pub struct EarlierBuild(usize);

fn contents(connection: &Connection) -> anyhow::Result<Contents> {
    let count = |sql: &str| -> anyhow::Result<u32> {
        let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
        Ok(u32::try_from(count)?)
    };
    Ok(Contents {
        // A song is live while one of its heads is (a deletion never beats a change).
        songs: count(
            "SELECT count(DISTINCT document_id) FROM versions
             WHERE kind = 'song' AND head = 1 AND body IS NOT NULL",
        )?,
        sets: count(
            "SELECT count(DISTINCT document_id) FROM versions
             WHERE kind = 'set' AND head = 1 AND body IS NOT NULL",
        )?,
    })
}

/// Backups sorted oldest first (names start with a zero-padded timestamp).
fn backups(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|entry| entry.ok().map(|entry| entry.path())).collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    files.retain(|path| path.extension().is_some_and(|extension| extension == "db"));
    files.sort();
    Ok(files)
}

/// The integrity check found damage.
#[derive(Debug, thiserror::Error)]
#[error("integrity check failed: {0}")]
struct Damaged(String);

/// True for errors that mean the file itself is damaged (and a backup should replace it).
fn is_damage(error: &anyhow::Error) -> bool {
    use rusqlite::ErrorCode;
    error.chain().any(|cause| {
        cause.is::<Damaged>()
            || matches!(
                cause.downcast_ref::<rusqlite::Error>(),
                Some(rusqlite::Error::SqliteFailure(failure, _))
                    if matches!(failure.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
            )
    })
}

fn prune_backups(dir: &Path) -> anyhow::Result<()> {
    let files = backups(dir)?;
    for old in files.iter().take(files.len().saturating_sub(BACKUPS_KEPT)) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// Moves the damaged database (with its WAL files) aside and copies the newest backup in place.
fn restore_newest_backup(data_dir: &Path) -> anyhow::Result<()> {
    let newest = backups(&data_dir.join(BACKUP_DIR))?
        .pop()
        .context("the database is damaged and there is no backup")?;
    let quarantine = data_dir.join("corrupt");
    std::fs::create_dir_all(&quarantine)?;
    let seconds =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for suffix in ["", "-wal", "-shm"] {
        let file = data_dir.join(format!("{DATABASE_FILE}{suffix}"));
        if file.exists() {
            std::fs::rename(&file, quarantine.join(format!("{seconds}-{DATABASE_FILE}{suffix}")))?;
        }
    }
    std::fs::copy(&newest, data_dir.join(DATABASE_FILE))?;
    tracing::warn!(backup = %newest.display(), "restored the database from a backup");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song, Verse};
    use sursum_protocol::SongId;

    fn song(text: &str) -> Song {
        Song {
            id: SongId::from("s1"),
            titles: vec!["Tytuł".into()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: vec![Verse::simple("v1", &[text])],
            extra: Default::default(),
        }
    }

    #[test]
    fn backups_rotate_and_a_damaged_database_is_restored() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut database = Database::open(dir.path()).unwrap();
            database.save_song(&song("zapisana"), Base::Current).unwrap();
            for index in 0..BACKUPS_KEPT + 3 {
                database.backup(dir.path(), &format!("t{index:02}")).unwrap();
            }
        }
        assert_eq!(backups(&dir.path().join(BACKUP_DIR)).unwrap().len(), BACKUPS_KEPT);
        // Simulate a damaged file: garbage over the whole database.
        std::fs::write(dir.path().join(DATABASE_FILE), vec![0x5a; 8192]).unwrap();
        let _ = std::fs::remove_file(dir.path().join("sursum.db-wal"));
        let database = Database::open(dir.path()).unwrap();
        assert_eq!(database.load_songs().unwrap()[0].verses[0].blocks[0].lines[0].text, "zapisana");
        assert!(dir.path().join("corrupt").read_dir().unwrap().next().is_some());
    }

    #[test]
    fn a_newer_schema_is_refused_but_never_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        {
            let database = Database::open(dir.path()).unwrap();
            database.backup(dir.path(), "startup").unwrap();
            database.connection.pragma_update(None, "user_version", 999).unwrap();
        }
        for _ in 0..3 {
            let error = Database::open(dir.path()).err().expect("refused").to_string();
            assert!(error.contains("newer"), "{error}");
        }
        assert!(!dir.path().join("corrupt").exists(), "the live database stays where it is");
        assert_eq!(Database::schema_version(dir.path()).unwrap(), 999);
        assert!(Database::backup_file(dir.path(), "before-x").unwrap().is_some());
    }

    #[test]
    fn sets_and_settings_round_trip() {
        let mut database = Database::in_memory().unwrap();
        let mut set = ServiceSet {
            id: SetId::from("set-1"),
            name: "Niedziela".into(),
            items: vec![SetItem {
                id: ItemId::from("i1"),
                kind: ItemKind::Text { title: "Ogł.".into(), slides: vec!["x".into()] },
            }],
            date: None,
            church: None,
        };
        database.save_set(&set).unwrap();
        database.save_set(&set).unwrap();
        set.items.clear();
        set.name = "Niedziela II".into();
        set.date = Some("2026-11-22".into());
        database.save_set(&set).unwrap();
        assert_eq!(database.load_sets().unwrap(), vec![set.clone()]);
        let heads = database.set_heads(&set.id).unwrap();
        assert_eq!(heads.current().unwrap().changes(), 1, "an unchanged save adds no version");
        database.delete_set("set-1").unwrap();
        assert!(database.load_sets().unwrap().is_empty());
        database.save_set(&set).unwrap();
        assert_eq!(database.load_sets().unwrap(), vec![set], "saving again undoes the deletion");
        database.set_setting("active_set", &"set-1").unwrap();
        assert_eq!(database.setting::<String>("active_set").unwrap().as_deref(), Some("set-1"));
        assert_eq!(database.setting::<String>("missing").unwrap(), None);
    }

    #[test]
    fn songs_are_versioned_and_unchanged_saves_write_nothing() {
        let mut database = Database::in_memory().unwrap();
        let SongSave::Created(first) = database.save_song(&song("a"), Base::Current).unwrap() else {
            panic!("a new song")
        };
        assert_eq!(
            database.save_song(&song("a"), Base::Current).unwrap(),
            SongSave::Unchanged(first.clone())
        );
        let SongSave::Changed(second) = database.save_song(&song("b"), Base::Loaded(&first)).unwrap() else {
            panic!("a new version")
        };
        assert_eq!(database.song_version("s1").unwrap(), Some(second));
        assert_eq!(
            database.save_song(&song("c"), Base::Loaded(&first)).unwrap(),
            SongSave::Outdated,
            "the editor's version was replaced meanwhile"
        );
        let songs = database.load_songs().unwrap();
        assert_eq!((songs.len(), songs[0].verses[0].blocks[0].lines[0].text.as_str()), (1, "b"));
        let versions: i64 =
            database.connection.query_row("SELECT count(*) FROM versions", [], |row| row.get(0)).unwrap();
        assert_eq!(versions, 2, "the first version stays as history");

        assert!(database.delete_song("s1").unwrap() && !database.delete_song("s1").unwrap());
        assert!(database.load_songs().unwrap().is_empty() && database.song_version("s1").unwrap().is_none());
        assert!(matches!(database.save_song(&song("wraca"), Base::Current).unwrap(), SongSave::Changed(_)));
        assert_eq!(database.load_songs().unwrap().len(), 1, "saving again brings it back");
    }

    #[test]
    fn a_song_body_has_no_id_and_sorted_keys() {
        let body = song_body(&song("a")).unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.starts_with(r#"{"authors":"#) && !text.contains(r#""id""#), "{text}");
        assert_eq!(song_from_body(&SongId::from("s1"), &body).unwrap(), song("a"));
    }

    #[test]
    fn a_database_of_an_earlier_build_is_refused_but_never_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        {
            let connection = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
            connection.execute_batch("CREATE TABLE songs (id TEXT PRIMARY KEY);").unwrap();
            connection.pragma_update(None, "user_version", 5).unwrap();
            connection.pragma_update(None, "application_id", APPLICATION_ID).unwrap();
        }
        let error = Database::open(dir.path()).err().expect("refused");
        assert!(error.chain().any(|cause| cause.is::<EarlierBuild>()), "{error:#}");
        assert!(!dir.path().join("corrupt").exists(), "an old database is not a damaged one");
        assert_eq!(Database::inspect(&dir.path().join(DATABASE_FILE)).err(), Some(Unreadable::Older));
    }

    fn saved(overlay: Overlay) -> SavedProgram {
        SavedProgram {
            set_id: Some(SetId::from("set")),
            item_id: Some(ItemId::from("item")),
            slide_key: Some(SlideKey::from("c#0@0")),
            overlay,
            gap: false,
        }
    }

    #[test]
    fn runtime_state_round_trips() {
        let database = Database::in_memory().unwrap();
        assert_eq!(database.load_runtime().unwrap(), None);
        database.save_runtime(&saved(Overlay::None), Some("boot-a")).unwrap();
        database.save_runtime(&saved(Overlay::Black), Some("boot-a")).unwrap();
        let record = database.load_runtime().unwrap().unwrap();
        assert_eq!(record.program, saved(Overlay::Black));
        assert_eq!(record.boot_id.as_deref(), Some("boot-a"));
    }

    #[test]
    fn reopening_keeps_data_and_does_not_rerun_migrations() {
        let dir = tempfile::tempdir().unwrap();
        Database::open(dir.path()).unwrap().save_runtime(&saved(Overlay::Logo), None).unwrap();
        let database = Database::open(dir.path()).unwrap();
        assert_eq!(database.load_runtime().unwrap().unwrap().program.overlay, Overlay::Logo);
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        drop(Database::open(dir.path()).unwrap());
        let connection = Connection::open(dir.path().join("sursum.db")).unwrap();
        connection.pragma_update(None, "user_version", Database::supported_schema() + 1).unwrap();
        drop(connection);
        assert!(Database::open(dir.path()).is_err());
    }
}
