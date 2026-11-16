// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSong import. Parsing is tolerant: unusual input produces warnings in the import report,
//! never a silent guess and never a crash. Only file reading in [`library`] touches the disk.

mod decode;
mod library;
mod lyrics;
mod markup;
mod song;
mod warning;

pub use decode::decode_text;
pub use library::{LibraryImport, add_file, is_skipped, read_library};
pub use lyrics::{ParsedLyrics, parse_lyrics, parse_presentation};
pub use markup::{OPTIONAL_BREAK, lyrics_markup, presentation_markup};
pub use song::{ImportedSong, SongError, parse_song, song_id_for_path};
pub use warning::{Warning, WarningKind};
