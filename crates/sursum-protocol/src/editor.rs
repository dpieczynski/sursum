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
