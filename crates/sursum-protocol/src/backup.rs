// SPDX-License-Identifier: GPL-3.0-or-later
//! Whole-device backups (`.sursum-backup`, ADR 0018): what a backup holds and the answer to
//! restoring one. Unlike a `.sursum` library file, which is merged song by song, a backup
//! replaces everything (songs, sets, settings, pictures) to bring a device back after a broken
//! SD card or a mistake.

use serde::{Deserialize, Serialize};

/// What a backup file holds, read from its manifest and database before anything is replaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSummary {
    /// When the backup was made, in seconds since the Unix epoch (the remote shows local time).
    pub created_unix: u64,
    /// Build id of the server that made it.
    pub build: String,
    /// Songs in the library (deleted ones not counted).
    pub songs: u32,
    /// Sets (deleted ones not counted).
    pub sets: u32,
    /// Stored pictures (logo and announcement posters).
    pub pictures: u32,
}

/// The answer to `POST /api/backup/restore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RestoreOutcome {
    /// The backup is staged; the server restarts right after answering and comes back with it.
    Restored { summary: BackupSummary },
    /// Nothing was changed.
    Refused { reason: RestoreRefusal },
}

/// Why a backup was not restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreRefusal {
    /// The server does not run under a service manager that would start it again.
    NotSupervised,
    /// Not a sursum backup, or damaged.
    NotABackup,
    /// Made by a newer version of the program, whose database this version cannot read.
    NewerVersion,
    /// Made by an earlier version whose data is not converted (ADR 0025).
    OlderVersion,
    /// Larger than any backup this device could have made.
    TooLarge,
    /// Writing failed (e.g. a full disk); the device keeps its current data.
    Failed,
}
