// SPDX-License-Identifier: GPL-3.0-or-later
//! Turning editor drafts into songs and previews. Parsing and slide splitting happen here, on
//! the server, so the browser never has to understand the markup.

use sursum_core::model::{Song, SongbookEntry};
use sursum_core::slides::{SlideRules, compile_song};
use sursum_formats::opensong::{
    Warning, WarningKind, editor_markup, lyrics_markup, markup_for_editor, parse_lyrics, parse_presentation,
    presentation_markup,
};
use sursum_protocol::{DraftPreview, ItemId, MarkupWarning, MarkupWarningCode, SongDraft, SongId};

/// Songbook name used for numbers entered in the editor when the song had none before.
const DEFAULT_SONGBOOK: &str = "Śpiewnik parafialny";

/// The editor's view of a stored song.
pub fn draft_of(song: &Song, version: &sursum_core::versions::VersionId) -> SongDraft {
    SongDraft {
        song_id: Some(song.id.clone()),
        base_version: Some(version.as_str().to_owned()),
        title: song.title().to_owned(),
        number: song.songbooks.first().map(|entry| entry.entry.clone()),
        presentation: presentation_markup(song),
        lyrics: markup_for_editor(&lyrics_markup(song)),
    }
}

/// Builds a song from a draft. Fields the editor does not show (authors, other titles,
/// imported extras) are kept from `existing`. Returns the song and the markup warnings.
pub fn song_from_draft(draft: &SongDraft, id: SongId, existing: Option<&Song>) -> (Song, Vec<MarkupWarning>) {
    let converted = editor_markup(&draft.lyrics);
    let parsed = parse_lyrics(&converted.markup);
    // The order typed in "Kolejność" wins; else the one implied by repeated refrains.
    let order = if draft.presentation.trim().is_empty() {
        converted.order.unwrap_or_default()
    } else {
        draft.presentation.clone()
    };
    let (verse_order, order_warnings) = parse_presentation(&order, &parsed.names);
    let warnings = parsed.warnings.iter().chain(&order_warnings).map(markup_warning).collect();

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

/// A parser warning as the remote gets it. Line numbers of plain text are not reported (the
/// converted markup has extra header lines), so a warning without a line is the safe choice.
fn markup_warning(warning: &Warning) -> MarkupWarning {
    let (code, detail) = match &warning.kind {
        WarningKind::LinesBeforeFirstSection => (MarkupWarningCode::LinesBeforeFirstSection, String::new()),
        WarningKind::DuplicateSection(name) => (MarkupWarningCode::DuplicateSection, name.clone()),
        WarningKind::UnknownSectionInPresentation(name) => {
            (MarkupWarningCode::UnknownSectionInOrder, name.clone())
        }
        other => (MarkupWarningCode::Other, other.to_string()),
    };
    MarkupWarning { line: warning.line.and_then(|line| u32::try_from(line).ok()), code, detail }
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

    fn any_version() -> sursum_core::versions::VersionId {
        sursum_core::versions::Version::first(Vec::new()).id().clone()
    }

    #[test]
    fn hidden_fields_survive_editing() {
        let (original, _) = song_from_draft(&draft("[V1]\n a"), SongId::from("s"), None);
        let mut original = original;
        original.authors = vec!["Autor".into()];
        original.titles.push("Inny tytuł".into());
        let mut edited = draft_of(&original, &any_version());
        edited.lyrics = "[V1]\n b".into();
        let (song, _) = song_from_draft(&edited, SongId::from("s"), Some(&original));
        assert_eq!(song.authors, ["Autor"]);
        assert_eq!(song.titles, ["Nowa pieśń", "Inny tytuł"]);
        assert_eq!(song.verses[0].blocks[0].lines[0].text, "b");
        assert_eq!(song.songbooks[0].entry, "42");
    }

    #[test]
    fn plain_text_becomes_verses_and_a_repeated_refrain() {
        let mut typed = draft("1. Pierwsza zwrotka\n\nRef. Refren\n\n2. Druga zwrotka\n\nRef.");
        typed.presentation = String::new();
        let preview = preview(&typed);
        let keys: Vec<&str> = preview.deck.slides.iter().map(|slide| slide.key.as_str()).collect();
        assert_eq!(keys, ["v1#0@0", "c#0@0", "v2#0@0", "c#0@1"]);
        assert!(preview.warnings.is_empty(), "{:?}", preview.warnings);
        let (song, _) = song_from_draft(&typed, SongId::from("s"), None);
        let shown = draft_of(&song, &any_version());
        assert!(shown.lyrics.starts_with("[V1]\nPierwsza zwrotka"), "no markup spaces: {}", shown.lyrics);
    }

    #[test]
    fn warnings_are_codes() {
        let mut with_typo = draft("[V1]\n zwrotka");
        with_typo.presentation = "V1 V9".into();
        let warnings = preview(&with_typo).warnings;
        assert_eq!(
            (warnings[0].code, warnings[0].detail.as_str()),
            (MarkupWarningCode::UnknownSectionInOrder, "V9")
        );
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
