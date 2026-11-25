// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSong import. Parsing is tolerant: unusual input produces warnings in the import report,
//! never a silent guess and never a crash. Only file reading in [`library`] touches the disk.

mod decode;
mod editor_text;
mod library;
mod lyrics;
mod markup;
mod set;
mod song;
mod warning;

pub use decode::{Decoding, decode_text};
pub use editor_text::{EditorMarkup, editor_markup, markup_for_editor};
pub use library::{FileFailure, LibraryImport, add_file, add_set_file, is_skipped, read_library, read_sets};
pub use lyrics::{ParsedLyrics, parse_lyrics, parse_presentation};
pub use markup::{OPTIONAL_BREAK, lyrics_markup, presentation_markup};
pub use set::{ImportedSet, SetEntry, SetError, SkipReason, SkippedGroup, parse_set, presentation_for};
pub use song::{ImportedSong, SongError, parse_song, song_id_for_path};
pub use warning::{Warning, WarningKind};
