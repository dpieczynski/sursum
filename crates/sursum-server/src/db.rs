// SPDX-License-Identifier: GPL-3.0-or-later
//! SQLite persistence. Only the state actor writes, so there is a single connection.
//!
//! Migrations are plain SQL scripts applied in order, tracked with `PRAGMA user_version`.
//! Durability settings follow the plan: WAL with `synchronous=FULL` (writes are rare, so an
//! fsync per commit costs nothing and survives power loss).

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song};
use sursum_protocol::{ItemId, Overlay, SetId, SlideKey};

/// Schema migrations; index + 1 is the resulting `user_version`. Never edit a released entry,
/// only append.
const MIGRATIONS: &[&str] = &[
    // 1: runtime state (the program position) and settings.
    "CREATE TABLE runtime_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        json TEXT NOT NULL,
        boot_id TEXT,
        saved_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        json TEXT NOT NULL
    );",
    // 2: the song library with a history of every revision (edits can be undone, libraries merged).
    "CREATE TABLE songs (
        id TEXT PRIMARY KEY,
        revision INTEGER NOT NULL,
        content_hash TEXT NOT NULL,
        title TEXT NOT NULL,
        body_json TEXT NOT NULL,
        source TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        deleted_at TEXT
    );
    CREATE TABLE song_history (
        song_id TEXT NOT NULL REFERENCES songs(id),
        revision INTEGER NOT NULL,
        content_hash TEXT NOT NULL,
        body_json TEXT NOT NULL,
        edited_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (song_id, revision)
    );",
    // 3: sets (running orders) and their items.
    "CREATE TABLE sets (
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
    );",
    // 4: paired remote devices (only a hash of each device's token is stored).
    "CREATE TABLE devices (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        token_hash TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        revoked_at TEXT
    );",
    // 5: content hashes of songs as other devices know them (from merged `.sursum` files), so a
    // stale file can be recognised as older and never resurrects or duplicates a song.
    "CREATE TABLE song_known_hashes (
        song_id TEXT NOT NULL,
        content_hash TEXT NOT NULL,
        PRIMARY KEY (song_id, content_hash)
    );",
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

/// What [`Database::upsert_song`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upsert {
    Inserted,
    Updated { revision: i64 },
    Unchanged,
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
        i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX)
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
        if version > MIGRATIONS.len() {
            anyhow::bail!(
                "database schema {version} is newer than this program ({}); refusing to start",
                MIGRATIONS.len()
            );
        }
        for (index, script) in MIGRATIONS.iter().enumerate().skip(version) {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(script).with_context(|| format!("applying migration {}", index + 1))?;
            transaction.pragma_update(None, "user_version", i64::try_from(index + 1)?)?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
            transaction.commit()?;
        }
        Ok(())
    }

    /// All songs that are not deleted.
    pub fn load_songs(&self) -> anyhow::Result<Vec<Song>> {
        let mut statement =
            self.connection.prepare("SELECT body_json FROM songs WHERE deleted_at IS NULL ORDER BY title")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|json| serde_json::from_str(&json?).context("decoding a stored song")).collect()
    }

    /// Inserts a song or, if its content changed, stores a new revision (keeping the old one in
    /// `song_history`). `source` records where it came from (e.g. `opensong:<path>`).
    pub fn upsert_song(&mut self, song: &Song, source: &str) -> anyhow::Result<Upsert> {
        let body = serde_json::to_string(song)?;
        let hash = content_hash(&body);
        let transaction = self.connection.savepoint()?;
        let existing: Option<(i64, String)> = transaction
            .query_row("SELECT revision, content_hash FROM songs WHERE id = ?1", [song.id.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        let outcome = match existing {
            Some((_, existing_hash)) if existing_hash == hash => Upsert::Unchanged,
            Some((revision, _)) => {
                let revision = revision + 1;
                transaction.execute(
                    "UPDATE songs SET revision = ?2, content_hash = ?3, title = ?4, body_json = ?5, source = ?6,
                     updated_at = datetime('now'), deleted_at = NULL WHERE id = ?1",
                    params![song.id.as_str(), revision, hash, song.title(), body, source],
                )?;
                Upsert::Updated { revision }
            }
            None => {
                transaction.execute(
                    "INSERT INTO songs (id, revision, content_hash, title, body_json, source) VALUES (?1, 1, ?2, ?3, ?4, ?5)",
                    params![song.id.as_str(), hash, song.title(), body, source],
                )?;
                Upsert::Inserted
            }
        };
        if outcome != Upsert::Unchanged {
            let revision = match outcome {
                Upsert::Updated { revision } => revision,
                _ => 1,
            };
            transaction.execute(
                "INSERT INTO song_history (song_id, revision, content_hash, body_json) VALUES (?1, ?2, ?3, ?4)",
                params![song.id.as_str(), revision, hash, body],
            )?;
        }
        transaction.commit()?;
        Ok(outcome)
    }

    /// All sets that are not deleted, items in order.
    pub fn load_sets(&self) -> anyhow::Result<Vec<ServiceSet>> {
        let mut sets_statement = self
            .connection
            .prepare("SELECT id, name FROM sets WHERE deleted_at IS NULL ORDER BY created_at, rowid")?;
        let sets: Vec<(String, String)> =
            sets_statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?;
        let mut items_statement = self
            .connection
            .prepare("SELECT item_id, kind_json FROM set_items WHERE set_id = ?1 ORDER BY position")?;
        let mut result = Vec::new();
        for (id, name) in sets {
            let rows: Vec<(String, String)> = items_statement
                .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let items = rows
                .into_iter()
                .map(|(item_id, json)| {
                    let kind: ItemKind = serde_json::from_str(&json).context("decoding a set item")?;
                    Ok(SetItem { id: ItemId(item_id), kind })
                })
                .collect::<anyhow::Result<_>>()?;
            result.push(ServiceSet { id: SetId(id), name, items });
        }
        Ok(result)
    }

    /// Stores a set, replacing its previous items.
    pub fn save_set(&mut self, set: &ServiceSet) -> anyhow::Result<()> {
        let transaction = self.connection.savepoint()?;
        transaction.execute(
            "INSERT INTO sets (id, name) VALUES (?1, ?2)
             ON CONFLICT (id) DO UPDATE SET name = excluded.name, updated_at = datetime('now')",
            params![set.id.as_str(), set.name],
        )?;
        transaction.execute("DELETE FROM set_items WHERE set_id = ?1", [set.id.as_str()])?;
        for (position, item) in set.items.iter().enumerate() {
            transaction.execute(
                "INSERT INTO set_items (set_id, position, item_id, kind_json) VALUES (?1, ?2, ?3, ?4)",
                params![
                    set.id.as_str(),
                    i64::try_from(position)?,
                    item.id.as_str(),
                    serde_json::to_string(&item.kind)?
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Marks a set as deleted (it stays in the database file, e.g. for a later undo tool).
    pub fn delete_set(&self, id: &str) -> anyhow::Result<()> {
        self.connection.execute(
            "UPDATE sets SET deleted_at = datetime('now') WHERE id = ?1 AND deleted_at IS NULL",
            [id],
        )?;
        Ok(())
    }

    /// Undoes [`Database::delete_set`].
    pub fn restore_set(&self, id: &str) -> anyhow::Result<()> {
        self.connection.execute("UPDATE sets SET deleted_at = NULL WHERE id = ?1", [id])?;
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

    /// Every song with its content hash, the hashes of all its revisions and whether it was
    /// deleted (deleted songs stay as tombstones, so a deletion can travel to another device).
    pub fn stored_songs(&self) -> anyhow::Result<Vec<StoredSong>> {
        let mut songs = self
            .connection
            .prepare("SELECT id, body_json, content_hash, deleted_at IS NOT NULL FROM songs ORDER BY id")?;
        // Own revisions first (in order), then hashes learned from other devices.
        let mut history = self.connection.prepare(
            "SELECT content_hash FROM song_history WHERE song_id = ?1
             UNION ALL
             SELECT content_hash FROM song_known_hashes WHERE song_id = ?1
               AND content_hash NOT IN (SELECT content_hash FROM song_history WHERE song_id = ?1)",
        )?;
        let rows: Vec<(String, String, String, bool)> = songs
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
            .collect::<Result<_, _>>()?;
        rows.into_iter()
            .map(|(id, body, content_hash, deleted)| {
                let song: Song = serde_json::from_str(&body).context("decoding a stored song")?;
                let history =
                    history.query_map([&id], |row| row.get(0))?.collect::<Result<Vec<String>, _>>()?;
                Ok(StoredSong { song, content_hash, history, deleted })
            })
            .collect()
    }

    /// Remembers hashes another device knew for a song (its history in a merged file).
    pub fn remember_hashes(&self, song_id: &str, hashes: &[String]) -> anyhow::Result<()> {
        let mut statement = self.connection.prepare_cached(
            "INSERT OR IGNORE INTO song_known_hashes (song_id, content_hash) VALUES (?1, ?2)",
        )?;
        for hash in hashes {
            statement.execute([song_id, hash])?;
        }
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

    /// Marks a song as deleted (it keeps its history and comes back if a newer version is
    /// imported or merged); false if there is no such live song.
    pub fn delete_song(&self, id: &str) -> anyhow::Result<bool> {
        let changed = self.connection.execute(
            "UPDATE songs SET deleted_at = datetime('now') WHERE id = ?1 AND deleted_at IS NULL",
            [id],
        )?;
        Ok(changed == 1)
    }

    /// The current revision of a song, if it exists.
    pub fn song_revision(&self, song_id: &str) -> anyhow::Result<Option<i64>> {
        let revision = self
            .connection
            .query_row("SELECT revision FROM songs WHERE id = ?1 AND deleted_at IS NULL", [song_id], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(revision)
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

/// A song as stored, for sharing libraries between devices.
#[derive(Debug, Clone)]
pub struct StoredSong {
    pub song: Song,
    pub content_hash: String,
    /// Hashes of every revision, oldest first (includes the current one).
    pub history: Vec<String>,
    pub deleted: bool,
}

/// The content hash the database stores for a song (to check hashes from other devices).
pub fn song_content_hash(song: &Song) -> anyhow::Result<String> {
    Ok(content_hash(&serde_json::to_string(song)?))
}

/// SHA-256 of a song's JSON, used to detect real changes.
fn content_hash(body: &str) -> String {
    Sha256::digest(body.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_core::model::Verse;
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
            database.upsert_song(&song("zapisana"), "test").unwrap();
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
        };
        database.save_set(&set).unwrap();
        set.items.clear();
        set.name = "Niedziela II".into();
        database.save_set(&set).unwrap();
        assert_eq!(database.load_sets().unwrap(), vec![set]);
        database.set_setting("active_set", &"set-1").unwrap();
        assert_eq!(database.setting::<String>("active_set").unwrap().as_deref(), Some("set-1"));
        assert_eq!(database.setting::<String>("missing").unwrap(), None);
    }

    #[test]
    fn songs_are_versioned_and_unchanged_imports_are_detected() {
        let mut database = Database::in_memory().unwrap();
        assert_eq!(database.upsert_song(&song("a"), "test").unwrap(), Upsert::Inserted);
        assert_eq!(database.upsert_song(&song("a"), "test").unwrap(), Upsert::Unchanged);
        assert_eq!(database.upsert_song(&song("b"), "test").unwrap(), Upsert::Updated { revision: 2 });
        let songs = database.load_songs().unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].verses[0].blocks[0].lines[0].text, "b");
        let history: i64 =
            database.connection.query_row("SELECT count(*) FROM song_history", [], |row| row.get(0)).unwrap();
        assert_eq!(history, 2);
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
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        assert!(Database::open(dir.path()).is_err());
    }
}
