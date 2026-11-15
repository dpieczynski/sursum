// SPDX-License-Identifier: GPL-3.0-or-later
//! SQLite persistence. Only the state actor writes, so there is a single connection.
//!
//! Migrations are plain SQL scripts applied in order, tracked with `PRAGMA user_version`.
//! Durability settings follow the plan: WAL with `synchronous=FULL` (writes are rare, so an
//! fsync per commit costs nothing and survives power loss).

use std::path::Path;

use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
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
];

/// `PRAGMA application_id` marking the file as a sursum database ("SURS").
const APPLICATION_ID: i64 = 0x5355_5253;

/// The program position saved after every change, restored at startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedProgram {
    pub set_id: Option<SetId>,
    pub item_id: Option<ItemId>,
    pub slide_key: Option<SlideKey>,
    pub overlay: Overlay,
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
    pub fn open(data_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_dir).with_context(|| format!("creating {}", data_dir.display()))?;
        let connection = Connection::open(data_dir.join("sursum.db"))?;
        Self::configure(connection)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(overlay: Overlay) -> SavedProgram {
        SavedProgram {
            set_id: Some(SetId::from("set")),
            item_id: Some(ItemId::from("item")),
            slide_key: Some(SlideKey::from("c#0@0")),
            overlay,
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
