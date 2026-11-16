// SPDX-License-Identifier: GPL-3.0-or-later
//! Writing a song back as OpenSong lyrics markup — the text format of the song editor, so
//! organists edit in the notation they know from OpenSong. Reading uses [`super::parse_lyrics`].
//!
//! One sursum extension: a line `---` marks an optional slide break (OpenLyrics
//! `break="optional"`), used only when a verse is too long for one slide.

use sursum_core::model::{BreakKind, Line, Song, Verse, VerseName};

/// The marker line for an optional break.
pub const OPTIONAL_BREAK: &str = "---";

/// The lyrics of `song` as OpenSong markup.
pub fn lyrics_markup(song: &Song) -> String {
    let mut out = String::new();
    for verse in &song.verses {
        out.push_str(&format!("[{}]\n", header(verse)));
        for block in &verse.blocks {
            for (index, line) in block.lines.iter().enumerate() {
                if !line.chords.is_empty() {
                    out.push_str(&chord_line(line));
                }
                out.push(' ');
                out.push_str(&line.text);
                let last = index + 1 == block.lines.len();
                if last && block.break_after == BreakKind::Forced {
                    out.push_str("||");
                }
                out.push('\n');
            }
            if block.break_after == BreakKind::Optional {
                out.push_str(OPTIONAL_BREAK);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out.trim_end().to_owned()
}

/// The presentation order as OpenSong writes it, e.g. `V1 C V2 C`.
pub fn presentation_markup(song: &Song) -> String {
    let headers: Vec<String> = song
        .verse_order
        .iter()
        .map(|name| song.verse(name).map_or_else(|| name_header(name), header))
        .collect();
    headers.join(" ")
}

/// The header of a verse: its custom label, or the upper-case name (`v1` → `V1`, `e` → `T`).
fn header(verse: &Verse) -> String {
    verse.label.clone().unwrap_or_else(|| name_header(&verse.name))
}

fn name_header(name: &VerseName) -> String {
    match name.0.strip_prefix('e') {
        Some(rest) => format!("T{}", rest.to_uppercase()),
        None => name.0.to_uppercase(),
    }
}

/// A `.` chord line whose chords sit above the given positions of the lyric line below.
fn chord_line(line: &Line) -> String {
    let mut out = String::from(".");
    let mut column = 0;
    for chord in &line.chords {
        while column < chord.position {
            out.push(' ');
            column += 1;
        }
        if column > chord.position {
            out.push(' ');
            column += 1;
        }
        out.push_str(&chord.chord);
        column += chord.chord.chars().count();
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opensong::{parse_lyrics, parse_presentation};

    fn round_trip(markup: &str) -> String {
        let parsed = parse_lyrics(markup);
        let song = Song {
            id: sursum_protocol::SongId::from("x"),
            titles: vec!["x".into()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: parsed.verses,
            extra: Default::default(),
        };
        lyrics_markup(&song)
    }

    #[test]
    fn markup_survives_a_round_trip() {
        let markup = "[V1]\n.G     C\n Amazing grace\n second line||\n after the break\n\n[C]\n refrain\n---\n more refrain\n\n[Intro]\n custom";
        assert_eq!(round_trip(markup), markup);
    }

    #[test]
    fn tag_is_written_as_t_and_presentation_uses_headers() {
        let parsed = parse_lyrics("[V1]\n a\n[T]\n end\n[Wstęp]\n x\n");
        let (order, _) = parse_presentation("Wstęp V1 T", &parsed.names);
        let song = Song {
            id: sursum_protocol::SongId::from("x"),
            titles: vec!["x".into()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: order,
            verses: parsed.verses,
            extra: Default::default(),
        };
        assert_eq!(presentation_markup(&song), "Wstęp V1 T");
        assert!(lyrics_markup(&song).contains("[T]\n end"));
    }
}
