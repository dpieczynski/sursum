// SPDX-License-Identifier: GPL-3.0-or-later
//! Turning editor drafts into songs and previews. Parsing and slide splitting happen here, on
//! the server, so the browser never has to understand the markup.

use sursum_core::model::{Song, SongbookEntry};
use sursum_core::slides::{SlideRules, compile_song};
use sursum_formats::opensong::{lyrics_markup, parse_lyrics, parse_presentation, presentation_markup};
use sursum_protocol::{DraftPreview, ItemId, SongDraft, SongId};

/// Songbook name used for numbers entered in the editor when the song had none before.
const DEFAULT_SONGBOOK: &str = "Śpiewnik parafialny";

/// The editor's view of a stored song.
pub fn draft_of(song: &Song, revision: u32) -> SongDraft {
    SongDraft {
        song_id: Some(song.id.clone()),
        base_revision: Some(revision),
        title: song.title().to_owned(),
        number: song.songbooks.first().map(|entry| entry.entry.clone()),
        presentation: presentation_markup(song),
        lyrics: lyrics_markup(song),
    }
}

/// Builds a song from a draft. Fields the editor does not show (authors, other titles,
/// imported extras) are kept from `existing`. Returns the song and the markup warnings.
pub fn song_from_draft(draft: &SongDraft, id: SongId, existing: Option<&Song>) -> (Song, Vec<String>) {
    let parsed = parse_lyrics(&draft.lyrics);
    let (verse_order, order_warnings) = parse_presentation(&draft.presentation, &parsed.names);
    let warnings = parsed.warnings.iter().chain(&order_warnings).map(ToString::to_string).collect();

    let mut song = existing.cloned().unwrap_or_else(|| Song {
        id: id.clone(),
        titles: Vec::new(),
        authors: Vec::new(),
        songbooks: Vec::new(),
        copyright: None,
        ccli: None,
        themes: Vec::new(),
        verse_order: Vec::new(),
        verses: Vec::new(),
        extra: Default::default(),
    });
    song.id = id;
    let other_titles = song.titles.iter().skip(1).cloned();
    song.titles = std::iter::once(draft.title.trim().to_owned()).chain(other_titles).collect();
    let number = draft.number.as_deref().map(str::trim).filter(|number| !number.is_empty());
    let songbook = song.songbooks.first().map_or(DEFAULT_SONGBOOK.to_owned(), |entry| entry.songbook.clone());
    song.songbooks =
        number.map(|entry| SongbookEntry { songbook, entry: entry.to_owned() }).into_iter().collect();
    song.verse_order = verse_order;
    song.verses = parsed.verses;
    (song, warnings)
}

/// The slides a draft produces.
pub fn preview(draft: &SongDraft) -> DraftPreview {
    let (song, warnings) = song_from_draft(draft, SongId::from("preview"), None);
    let deck = compile_song(&song, &ItemId::from("preview"), None, SlideRules::default());
    DraftPreview { deck, warnings }
}

/// Why a draft cannot be saved, if it cannot.
pub fn validation_error(song: &Song) -> Option<&'static str> {
    if song.title().is_empty() {
        Some("title_missing")
    } else if song.verses.is_empty() {
        Some("lyrics_missing")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(lyrics: &str) -> SongDraft {
        SongDraft {
            title: " Nowa pieśń ".into(),
            number: Some("42".into()),
            presentation: "V1 C V1".into(),
            lyrics: lyrics.into(),
            ..SongDraft::default()
        }
    }

    #[test]
    fn preview_compiles_slides_in_presentation_order() {
        let preview = preview(&draft("[V1]\n zwrotka\n[C]\n refren"));
        let keys: Vec<&str> = preview.deck.slides.iter().map(|slide| slide.key.as_str()).collect();
        assert_eq!(keys, ["v1#0@0", "c#0@0", "v1#0@1"]);
        assert!(preview.warnings.is_empty());
    }

    #[test]
    fn warnings_are_reported() {
        let mut with_typo = draft("[V1]\n zwrotka");
        with_typo.presentation = "V1 V9".into();
        assert_eq!(preview(&with_typo).warnings.len(), 1);
    }

    #[test]
    fn hidden_fields_survive_editing() {
        let (original, _) = song_from_draft(&draft("[V1]\n a"), SongId::from("s"), None);
        let mut original = original;
        original.authors = vec!["Autor".into()];
        original.titles.push("Inny tytuł".into());
        let mut edited = draft_of(&original, 1);
        edited.lyrics = "[V1]\n b".into();
        let (song, _) = song_from_draft(&edited, SongId::from("s"), Some(&original));
        assert_eq!(song.authors, ["Autor"]);
        assert_eq!(song.titles, ["Nowa pieśń", "Inny tytuł"]);
        assert_eq!(song.verses[0].blocks[0].lines[0].text, "b");
        assert_eq!(song.songbooks[0].entry, "42");
    }

    #[test]
    fn empty_title_or_lyrics_cannot_be_saved() {
        let (no_lyrics, _) = song_from_draft(&draft(""), SongId::from("s"), None);
        assert_eq!(validation_error(&no_lyrics), Some("lyrics_missing"));
        let mut untitled = draft("[V1]\n a");
        untitled.title = "  ".into();
        let (untitled, _) = song_from_draft(&untitled, SongId::from("s"), None);
        assert_eq!(validation_error(&untitled), Some("title_missing"));
    }
}
