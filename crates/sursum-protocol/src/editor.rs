// SPDX-License-Identifier: GPL-3.0-or-later
//! The song editor's REST payloads. The lyrics travel as OpenSong markup; the server parses and
//! compiles them (the browser never interprets the markup).

use serde::{Deserialize, Serialize};

use crate::ids::{ItemId, SongId};
use crate::slide::Deck;
use crate::state::SetItemKind;

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
    /// Warnings and failures (at most a few hundred), as codes the remote words.
    pub problems: Vec<ImportProblem>,
}

/// One problem an import met: where, and what (as a code with its data, so the remote can word
/// it in its language).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProblem {
    /// The file in the OpenSong folder: a song's path inside `Songs/` (e.g. `Maryjne/Barka`) or
    /// `Sets/<name>`. Empty for a problem with the whole import.
    pub file: String,
    /// 1-based line of the song's lyrics, if the problem is on one line.
    #[serde(default)]
    pub line: Option<u32>,
    pub code: ImportProblemCode,
}

/// What an [`ImportProblem`] is. Song warnings and set notes leave the file imported (the
/// variant says what was done instead); failures skip the file or the set entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportProblemCode {
    /// Lyrics before the first `[section]`; they became verse 1.
    LinesBeforeFirstSection,
    /// A lyric line that starts with `character` instead of a space; kept as text.
    UnexpectedLineStart { character: char },
    /// The same `[section]` twice; the second was appended to the first with a slide break.
    DuplicateSection { section: String },
    /// The song's presentation order names a section it does not have; skipped.
    UnknownSectionInOrder { section: String },
    /// Not UTF-8 and no other encoding declared; read as windows-1250 (the usual Polish one).
    AssumedWindows1250,
    /// Bytes invalid in the file's encoding were replaced with U+FFFD.
    InvalidBytes,
    /// An element that should appear once (e.g. `title`) appears again with other text; the
    /// first was used.
    RepeatedElement { element: String },
    /// No `<title>`; the file name was used.
    TitleFromFileName,
    /// The song has no lyrics.
    NoLyrics,
    /// The file is not an OpenSong song (in `Songs/`) or set (in `Sets/`); skipped.
    NotOpenSong,
    /// The file is not valid XML; skipped. `detail` is the parser's (English) message.
    InvalidXml { detail: String },
    /// The file could not be read; skipped. `detail` is the system's message.
    Unreadable { detail: String },
    /// A set group with no slides; skipped.
    EmptyGroup { group: String },
    /// A set group of a kind sursum does not import (e.g. `image`); skipped.
    UnsupportedGroup { group: String, group_type: String },
    /// A set names a song that is not in the library; skipped.
    SongNotFound { song: String },
    /// Several songs have the name a set uses; the one at `used` (the shortest path) was taken.
    AmbiguousSong { song: String, count: u32, used: String },
    /// A set's order for `song` names a section the song does not have; skipped.
    UnknownSectionInSetOrder { song: String, section: String },
    /// Nothing could be stored (e.g. the disk is full); the import changed nothing.
    StorageFailed,
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
    /// This church's sets the file added (ADR 0025).
    #[serde(default)]
    pub sets_added: u32,
    /// This church's sets the file changed (or deleted).
    #[serde(default)]
    pub sets_updated: u32,
    /// Names of this church's sets changed in two places, waiting for review.
    #[serde(default)]
    pub sets_to_review: Vec<String>,
    /// Sets of other churches the file added or changed: kept and passed on, not shown here.
    #[serde(default)]
    pub other_church_sets: u32,
    /// Pictures the file brought that this device did not have.
    #[serde(default)]
    pub pictures_added: u32,
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
    /// Best first, by the rule that picks the shown version (ADR 0025: more changes win): the
    /// version every device shows now comes first, the others follow in the same order on every
    /// device.
    pub versions: Vec<SongVersionView>,
}

/// One version of a song, as the comparison screen shows it: every field the editor saves, so
/// versions that differ only in the songbook number or the order do not look the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongVersionView {
    pub version: String,
    /// True for the version every device shows now (the one with more changes).
    pub shown: bool,
    pub title: String,
    /// Number in the songbook, if any.
    pub number: Option<String>,
    /// Presentation order as the editor shows it, e.g. `V1 C V2 C`; empty means "as written".
    pub presentation: String,
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

/// `GET /api/sets/{id}/versions`: a set changed in two places (ADR 0025), every live version it
/// holds, for the comparison screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetVersions {
    pub set_id: crate::ids::SetId,
    /// Best first, like [`SongVersions::versions`]: the version every device shows now comes
    /// first.
    pub versions: Vec<SetVersionView>,
}

/// One version of a set, as the comparison screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetVersionView {
    pub version: String,
    /// True for the version every device shows now (the one with more changes).
    pub shown: bool,
    pub name: String,
    pub date: Option<String>,
    pub items: Vec<SetVersionItem>,
}

/// An item of one version of a set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetVersionItem {
    pub item_id: ItemId,
    pub title: String,
    pub kind: SetItemKind,
    /// The song's number in its songbook, if any (as in [`crate::SetItemView`]).
    #[serde(default)]
    pub song_number: Option<String>,
    /// Another version lacks this item: the comparison offers "Dodaj" next to it.
    pub missing_elsewhere: bool,
}
