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
    /// The version the editor started from (ADR 0025): saving builds on it, and is refused if
    /// it was replaced meanwhile. `None` for a new song.
    pub base_version: Option<String>,
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
    pub warnings: Vec<MarkupWarning>,
}

/// A problem in the editor's text, as a code the remote puts into words (in its language).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkupWarning {
    /// 1-based line of the editor's text, if the problem is on one line.
    pub line: Option<u32>,
    pub code: MarkupWarningCode,
    /// The section or character involved, e.g. `V9`.
    #[serde(default)]
    pub detail: String,
}

/// What kind of problem a [`MarkupWarning`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkupWarningCode {
    /// Text before the first `[section]`; it became verse 1.
    LinesBeforeFirstSection,
    /// The same `[section]` twice; the second was appended to the first.
    DuplicateSection,
    /// The order names a section that does not exist; it was skipped.
    UnknownSectionInOrder,
    /// Anything else (rare; the detail says what).
    Other,
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
    /// Titles of songs whose file text differs from a correction made in sursum since the last
    /// import (ADR 0025): both versions stay, the song waits for review.
    #[serde(default)]
    pub to_review: Vec<String>,
    /// Warnings and failures, one line each (at most a few hundred).
    pub problems: Vec<String>,
}

/// What merging a `.sursum` library file does (or would do, in a dry run).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeReport {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    /// Songs where this device already knows everything the file holds (or more).
    pub kept_local: u32,
    /// Songs deleted on the other device in a version this device had.
    #[serde(default)]
    pub deleted: u32,
    /// Titles of songs changed in both places (ADR 0025): the version with more changes is
    /// shown on every device, the other one stays with the song and waits for review.
    pub conflicts: Vec<String>,
}

/// Result of saving a draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SaveOutcome {
    Saved {
        song_id: SongId,
        /// The version just saved; the editor builds its next save on it.
        version: String,
    },
    /// Someone saved this song since the editor opened it; nothing was written.
    Conflict,
    /// The draft cannot be saved (e.g. no title or no lyrics).
    Invalid { reason: String },
}

/// A song changed in two places (ADR 0025): every live version it holds, for the comparison
/// screen. The server puts each version into the editor's text form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongVersions {
    pub song_id: SongId,
    /// The version every device shows now comes first.
    pub versions: Vec<SongVersionView>,
}

/// One version of a song, as the comparison screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongVersionView {
    pub version: String,
    /// True for the version every device shows now (the one with more changes).
    pub shown: bool,
    pub title: String,
    /// The lyrics in the editor's text form.
    pub lyrics: String,
}

/// `POST /api/songs/{id}/keep`: keep this version and replace all others (the review's
/// "Zostaw tę"). Answers a [`SaveOutcome`]; `Conflict` if the version is not one of the song's
/// versions any more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeepVersion {
    pub version: String,
}
