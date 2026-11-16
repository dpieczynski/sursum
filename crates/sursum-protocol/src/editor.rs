// SPDX-License-Identifier: GPL-3.0-or-later
//! The song editor's REST payloads. The lyrics travel as OpenSong markup; the server parses and
//! compiles them (the browser never interprets the markup).

use serde::{Deserialize, Serialize};

use crate::ids::SongId;
use crate::slide::Deck;

/// A song in the editor's text form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongDraft {
    /// `None` for a new song.
    pub song_id: Option<SongId>,
    /// The revision the editor started from (optimistic locking); `None` for a new song.
    pub base_revision: Option<u32>,
    pub title: String,
    /// Number in the songbook, if any.
    pub number: Option<String>,
    /// Presentation order, e.g. `V1 C V2 C`; empty means "as written".
    pub presentation: String,
    /// Lyrics in OpenSong markup (`[V1]`, ` line`, `||`, `---`).
    pub lyrics: String,
}

/// The slides a draft would produce, plus problems found in the markup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftPreview {
    pub deck: Deck,
    pub warnings: Vec<String>,
}

/// What an import of songs did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub inserted: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub failed: u32,
    /// Sets imported from the `Sets/` folder (re-importing a set replaces it).
    #[serde(default)]
    pub sets: u32,
    /// Warnings and failures, one line each (at most a few hundred).
    pub problems: Vec<String>,
}

/// What merging a `.sursum` library file does (or would do, in a dry run).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeReport {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    /// Songs where this device already has a newer version.
    pub kept_local: u32,
    /// Titles of songs changed on both sides; the incoming version is stored as a copy.
    pub conflicts: Vec<String>,
}

/// Result of saving a draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SaveOutcome {
    Saved {
        song_id: SongId,
        revision: u32,
    },
    /// Someone saved this song since the editor opened it; nothing was written.
    Conflict {
        current_revision: u32,
    },
    /// The draft cannot be saved (e.g. no title or no lyrics).
    Invalid {
        reason: String,
    },
}
