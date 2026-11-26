// SPDX-License-Identifier: GPL-3.0-or-later
//! The slide compiler: turns a song (or a text, image or blank item) into a [`Deck`] of slides.
//!
//! Rules:
//! - every verse starts a new slide;
//! - a forced break (OpenSong `||`) always starts a new slide;
//! - a verse longer than `max_lines` is split, first at optional breaks, then into
//!   equally long chunks (never a lonely last line).

use sha2::{Digest, Sha256};
use sursum_protocol::{
    AssetId, Deck, DeckId, ItemId, SectionKind, SectionName, Slide, SlideBody, SlideKey, SlidePart, SongId,
};

#[cfg(test)]
use crate::model::Line;
use crate::model::{BreakKind, ItemKind, LinesBlock, SetItem, Song, Verse, VerseName};

/// Settings that control how verses are split into slides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideRules {
    /// Maximum number of lines on one slide.
    pub max_lines: usize,
}

impl Default for SlideRules {
    fn default() -> Self {
        SlideRules { max_lines: 6 }
    }
}

/// Compiles a song into a deck. `order_override` is a set-specific verse order.
///
/// Verses named in the order but missing from the song are skipped.
pub fn compile_song(
    song: &Song,
    item_id: &ItemId,
    order_override: Option<&[VerseName]>,
    rules: SlideRules,
) -> Deck {
    let order = presentation_order(song, order_override);
    let mut slides = Vec::new();
    for (position, name) in order.iter().enumerate() {
        let Some(verse) = song.verse(name) else {
            continue;
        };
        let occurrence = order[..position].iter().filter(|earlier| *earlier == name).count();
        slides.extend(verse_slides(verse, occurrence, rules));
    }
    build_deck(item_id, song.title(), slides)
}

/// Compiles a text item: each entry of `pages` is one slide, lines separated by `\n`.
pub fn compile_text(item_id: &ItemId, title: &str, pages: &[String]) -> Deck {
    let slides = pages
        .iter()
        .enumerate()
        .map(|(index, page)| Slide {
            key: SlideKey(format!("t#{index}")),
            part: SlidePart::Page { number: u32::try_from(index + 1).unwrap_or(u32::MAX) },
            body: SlideBody::Text { title: None, lines: page.lines().map(str::to_owned).collect() },
        })
        .collect();
    build_deck(item_id, title, slides)
}

/// Compiles any set item; `None` if it names a song that is not in the library.
pub fn compile_item<'a>(
    item: &SetItem,
    song: impl Fn(&SongId) -> Option<&'a Song>,
    rules: SlideRules,
) -> Option<Deck> {
    Some(match &item.kind {
        ItemKind::Song { song_id, order_override } => {
            compile_song(song(song_id)?, &item.id, order_override.as_deref(), rules)
        }
        ItemKind::Text { title, slides } => compile_text(&item.id, title, slides),
        ItemKind::Image { title, asset_id } => compile_image(&item.id, title, asset_id),
        ItemKind::Blank { title } => compile_blank(&item.id, title),
    })
}

/// Compiles an image item: one slide showing the picture.
pub fn compile_image(item_id: &ItemId, title: &str, asset_id: &AssetId) -> Deck {
    let slide = Slide {
        key: SlideKey("img#0".to_owned()),
        part: SlidePart::Picture,
        body: SlideBody::Image { asset_id: asset_id.clone() },
    };
    build_deck(item_id, title, vec![slide])
}

/// Compiles a pause: one empty slide (only the background shows).
pub fn compile_blank(item_id: &ItemId, title: &str) -> Deck {
    let slide = Slide {
        key: SlideKey("blank#0".to_owned()),
        part: SlidePart::Pause,
        body: SlideBody::Text { title: None, lines: Vec::new() },
    };
    build_deck(item_id, title, vec![slide])
}

/// The verse order to present: the override, else the song's order, else written order.
fn presentation_order(song: &Song, order_override: Option<&[VerseName]>) -> Vec<VerseName> {
    match order_override {
        Some(order) if !order.is_empty() => order.to_vec(),
        _ if !song.verse_order.is_empty() => song.verse_order.clone(),
        _ => song.verses.iter().map(|verse| verse.name.clone()).collect(),
    }
}

/// Slides for one occurrence of a verse in the presentation order.
fn verse_slides(verse: &Verse, occurrence: usize, rules: SlideRules) -> Vec<Slide> {
    let section = section_name(verse);
    split_verse(&verse.blocks, rules.max_lines)
        .into_iter()
        .enumerate()
        .map(|(chunk, lines)| Slide {
            key: SlideKey(format!("{}#{chunk}@{occurrence}", verse.name.0)),
            part: SlidePart::Section {
                section: section.clone(),
                piece: u32::try_from(chunk).unwrap_or(u32::MAX),
            },
            body: SlideBody::Lyrics { lines },
        })
        .collect()
}

/// What the remote needs to name a verse: its kind, number and letter from the verse name
/// (`v2` → verse 2, `c` → chorus, `v1a` → verse 1, letter a), or its own label. A name that does
/// not follow that pattern (`x9z1`, possible only in hand-made files) or whose number does not
/// fit a `u32` is an "other" section labelled with the name, so nothing is lost. A blank label
/// (`[]` in OpenSong) or a blank name counts as none, so the remote never shows an empty name.
pub fn section_name(verse: &Verse) -> SectionName {
    let name = verse.name.0.as_str();
    let mut chars = name.chars();
    let kind = match chars.next() {
        Some('v') => Some(SectionKind::Verse),
        Some('c') => Some(SectionKind::Chorus),
        Some('b') => Some(SectionKind::Bridge),
        Some('p') => Some(SectionKind::PreChorus),
        Some('e') => Some(SectionKind::Ending),
        Some('i') => Some(SectionKind::Intro),
        Some('o') => Some(SectionKind::Other),
        _ => None,
    };
    let rest = chars.as_str();
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let mut suffix = rest[digits.len()..].chars();
    // `Some(None)`: no number (`c`); `None`: not a number we can keep (`x9`, an overflow).
    let number = match digits {
        "" => Some(None),
        digits if digits.chars().all(|c| c.is_ascii_digit()) => digits.parse::<u32>().ok().map(Some),
        _ => None,
    };
    let label = custom_label(verse);
    match (kind, number, suffix.next(), suffix.next()) {
        (Some(kind), Some(number), letter, None) => SectionName { kind, number, suffix: letter, label },
        _ => SectionName {
            kind: SectionKind::Other,
            number: None,
            suffix: None,
            label: label.or_else(|| Some(name.trim().to_owned()).filter(|name| !name.is_empty())),
        },
    }
}

/// The verse's own label, unless it is blank.
fn custom_label(verse: &Verse) -> Option<String> {
    verse.label.clone().filter(|label| !label.trim().is_empty())
}

/// Splits the lines of a verse into slide-sized chunks.
fn split_verse(blocks: &[LinesBlock], max_lines: usize) -> Vec<Vec<String>> {
    let max_lines = max_lines.max(1);
    let mut chunks = Vec::new();
    for segment in forced_segments(blocks) {
        let total: usize = segment.iter().map(|block| block.lines.len()).sum();
        if total == 0 {
            continue;
        }
        if total <= max_lines {
            chunks.push(segment.iter().flat_map(|block| block.texts().map(str::to_owned)).collect());
            continue;
        }
        for group in optional_groups(&segment, max_lines) {
            chunks.extend(split_evenly(&group, max_lines));
        }
    }
    chunks
}

/// Groups blocks into segments separated by forced breaks.
fn forced_segments(blocks: &[LinesBlock]) -> Vec<Vec<&LinesBlock>> {
    let mut segments = vec![Vec::new()];
    for block in blocks {
        segments.last_mut().expect("segments is never empty").push(block);
        if block.break_after == BreakKind::Forced {
            segments.push(Vec::new());
        }
    }
    segments.retain(|segment| !segment.is_empty());
    segments
}

/// Greedily packs blocks into groups of at most `max_lines`, splitting only at optional breaks.
fn optional_groups(segment: &[&LinesBlock], max_lines: usize) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for block in segment {
        let fits = current.len() + block.lines.len() <= max_lines;
        if !current.is_empty() && !fits {
            groups.push(std::mem::take(&mut current));
        }
        current.extend(block.texts().map(str::to_owned));
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Splits lines into the fewest chunks of at most `max_lines`, as equal in length as possible
/// (longer chunks first), so a slide never ends up with a single orphaned line.
fn split_evenly(lines: &[String], max_lines: usize) -> Vec<Vec<String>> {
    if lines.len() <= max_lines {
        return vec![lines.to_vec()];
    }
    let chunk_count = lines.len().div_ceil(max_lines);
    let base = lines.len() / chunk_count;
    let longer = lines.len() % chunk_count;
    let mut chunks = Vec::with_capacity(chunk_count);
    let mut start = 0;
    for index in 0..chunk_count {
        let size = base + usize::from(index < longer);
        chunks.push(lines[start..start + size].to_vec());
        start += size;
    }
    chunks
}

/// Wraps slides into a deck with a content-derived id.
fn build_deck(item_id: &ItemId, title: &str, slides: Vec<Slide>) -> Deck {
    let deck_id = content_id(title, &slides);
    Deck { fit_group: deck_id.0.clone(), deck_id, item_id: item_id.clone(), title: title.to_owned(), slides }
}

/// A short, stable hash of everything that affects how the deck looks.
fn content_id(title: &str, slides: &[Slide]) -> DeckId {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update([0]);
    for slide in slides {
        hasher.update(slide.key.0.as_bytes());
        hasher.update([0]);
        let body = match &slide.body {
            SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => lines.join("\n"),
            SlideBody::Image { asset_id } => asset_id.0.clone(),
        };
        hasher.update(body.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    DeckId(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_protocol::SongId;

    fn lines(count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("linia {n}")).collect()
    }

    fn block(count: usize, break_after: BreakKind) -> LinesBlock {
        LinesBlock { lines: lines(count).into_iter().map(Line::text).collect(), break_after }
    }

    fn song(verses: Vec<Verse>, order: &[&str]) -> Song {
        Song {
            id: SongId::from("song"),
            titles: vec!["Pieśń".to_owned()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            extra: Default::default(),
            verse_order: order.iter().map(|name| VerseName::new(name)).collect(),
            verses,
        }
    }

    fn keys(deck: &Deck) -> Vec<&str> {
        deck.slides.iter().map(|slide| slide.key.as_str()).collect()
    }

    #[test]
    fn each_verse_becomes_a_slide_in_written_order() {
        let song = song(
            vec![Verse::simple("v1", &["a", "b"]), Verse::simple("c", &["r"]), Verse::simple("v2", &["c"])],
            &[],
        );
        let deck = compile_song(&song, &ItemId::from("i"), None, SlideRules::default());
        assert_eq!(keys(&deck), ["v1#0@0", "c#0@0", "v2#0@0"]);
    }

    #[test]
    fn presentation_order_repeats_and_counts_occurrences() {
        let song = song(
            vec![Verse::simple("v1", &["a"]), Verse::simple("c", &["r"]), Verse::simple("v2", &["b"])],
            &["v1", "c", "v2", "c"],
        );
        let deck = compile_song(&song, &ItemId::from("i"), None, SlideRules::default());
        assert_eq!(keys(&deck), ["v1#0@0", "c#0@0", "v2#0@0", "c#0@1"]);
    }

    #[test]
    fn override_order_wins_and_missing_verses_are_skipped() {
        let song = song(vec![Verse::simple("v1", &["a"]), Verse::simple("c", &["r"])], &["v1", "c"]);
        let order = [VerseName::new("c"), VerseName::new("v9"), VerseName::new("v1")];
        let deck = compile_song(&song, &ItemId::from("i"), Some(&order), SlideRules::default());
        assert_eq!(keys(&deck), ["c#0@0", "v1#0@0"]);
    }

    #[test]
    fn forced_break_always_splits() {
        let verse = Verse {
            name: VerseName::new("v1"),
            label: None,
            blocks: vec![block(2, BreakKind::Forced), block(2, BreakKind::None)],
        };
        let deck = compile_song(&song(vec![verse], &[]), &ItemId::from("i"), None, SlideRules::default());
        assert_eq!(keys(&deck), ["v1#0@0", "v1#1@0"]);
    }

    #[test]
    fn optional_break_splits_only_when_too_long() {
        let verse = |count| Verse {
            name: VerseName::new("v1"),
            label: None,
            blocks: vec![block(count, BreakKind::Optional), block(count, BreakKind::None)],
        };
        let verse_one = SectionName { kind: SectionKind::Verse, number: Some(1), suffix: None, label: None };
        let piece = |piece| SlidePart::Section { section: verse_one.clone(), piece };
        let short = compile_song(&song(vec![verse(2)], &[]), &ItemId::from("i"), None, SlideRules::default());
        let parts: Vec<&SlidePart> = short.slides.iter().map(|slide| &slide.part).collect();
        assert_eq!(parts, [&piece(0)]);
        let long = compile_song(&song(vec![verse(4)], &[]), &ItemId::from("i"), None, SlideRules::default());
        let parts: Vec<&SlidePart> = long.slides.iter().map(|slide| &slide.part).collect();
        assert_eq!(parts, [&piece(0), &piece(1)]);
    }

    #[test]
    fn long_verse_is_split_evenly_without_orphans() {
        let chunks = split_evenly(&lines(7), 6);
        let sizes: Vec<usize> = chunks.iter().map(Vec::len).collect();
        assert_eq!(sizes, [4, 3]);
    }

    #[test]
    fn deck_id_changes_with_content_only() {
        let a = song(vec![Verse::simple("v1", &["a"])], &[]);
        let mut b = a.clone();
        let id = |song: &Song| compile_song(song, &ItemId::from("i"), None, SlideRules::default()).deck_id;
        assert_eq!(id(&a), id(&b));
        b.verses[0].blocks[0].lines[0] = Line::text("zmiana");
        assert_ne!(id(&a), id(&b));
    }

    #[test]
    fn every_slide_says_which_section_and_piece_it_is() {
        let long = Verse {
            name: VerseName::new("c"),
            label: None,
            blocks: vec![block(2, BreakKind::Forced), block(2, BreakKind::None)],
        };
        // The chorus comes twice: each time its pieces count from 0 again.
        let song = song(vec![Verse::simple("v2", &["a"]), long], &["v2", "c", "c"]);
        let deck = compile_song(&song, &ItemId::from("i"), None, SlideRules::default());
        assert_eq!(keys(&deck), ["v2#0@0", "c#0@0", "c#1@0", "c#0@1", "c#1@1"]);
        let parts: Vec<&SlidePart> = deck.slides.iter().map(|slide| &slide.part).collect();
        let section = |kind, number| SectionName { kind, number, suffix: None, label: None };
        let verse = |piece| SlidePart::Section { section: section(SectionKind::Verse, Some(2)), piece };
        let chorus = |piece| SlidePart::Section { section: section(SectionKind::Chorus, None), piece };
        assert_eq!(parts, [&verse(0), &chorus(0), &chorus(1), &chorus(0), &chorus(1)]);
    }

    #[test]
    fn section_names_keep_letters_and_custom_labels() {
        let name = |verse: &str| section_name(&Verse::simple(verse, &["a"]));
        assert_eq!(
            name("v1a"),
            SectionName { kind: SectionKind::Verse, number: Some(1), suffix: Some('a'), label: None }
        );
        assert_eq!(name("e").kind, SectionKind::Ending);
        let mut custom = Verse::simple("o1", &["a"]);
        custom.label = Some("Alleluja".to_owned());
        assert_eq!(section_name(&custom).label.as_deref(), Some("Alleluja"));
        assert_eq!(
            name("x9z1"),
            SectionName { kind: SectionKind::Other, number: None, suffix: None, label: Some("x9z1".into()) },
            "an odd name is shown as written"
        );
    }

    #[test]
    fn a_blank_custom_label_is_no_label() {
        for blank in ["", "  "] {
            let mut verse = Verse::simple("o1", &["a"]);
            verse.label = Some(blank.to_owned());
            assert_eq!(
                section_name(&verse),
                SectionName { kind: SectionKind::Other, number: Some(1), suffix: None, label: None },
                "label {blank:?}"
            );
        }
        let mut odd = Verse::simple("x9z1", &["a"]);
        odd.label = Some(" ".to_owned());
        assert_eq!(section_name(&odd).label.as_deref(), Some("x9z1"), "an odd name falls back to the name");
        for blank in ["", "  "] {
            assert_eq!(
                section_name(&Verse::simple(blank, &["a"])),
                SectionName { kind: SectionKind::Other, number: None, suffix: None, label: None },
                "name {blank:?}"
            );
        }
    }

    #[test]
    fn a_number_too_big_to_keep_is_shown_as_written() {
        let verse = Verse::simple("v12345678901", &["a"]);
        assert_eq!(
            section_name(&verse),
            SectionName {
                kind: SectionKind::Other,
                number: None,
                suffix: None,
                label: Some("v12345678901".into())
            }
        );
        let mut labelled = Verse::simple("v12345678901", &["a"]);
        labelled.label = Some("Zwrotka długa".to_owned());
        assert_eq!(section_name(&labelled).label.as_deref(), Some("Zwrotka długa"));
    }

    #[test]
    fn text_items_become_one_slide_per_page() {
        let deck = compile_text(
            &ItemId::from("t"),
            "Ogłoszenia",
            &["Pierwsza\nlinia".to_owned(), "Druga".to_owned()],
        );
        assert_eq!(keys(&deck), ["t#0", "t#1"]);
        assert_eq!(deck.slides[1].part, SlidePart::Page { number: 2 });
        assert_eq!(
            deck.slides[0].body,
            SlideBody::Text { title: None, lines: vec!["Pierwsza".to_owned(), "linia".to_owned()] }
        );
    }

    proptest::proptest! {
        #[test]
        fn split_evenly_keeps_every_line_and_respects_the_limit(count in 0usize..60, max in 1usize..10) {
            let input = lines(count);
            let chunks = split_evenly(&input, max);
            let flat: Vec<String> = chunks.iter().flatten().cloned().collect();
            proptest::prop_assert_eq!(flat, input);
            for chunk in &chunks {
                proptest::prop_assert!(chunk.len() <= max.max(count.min(max)));
            }
            if count > max {
                let sizes: Vec<usize> = chunks.iter().map(Vec::len).collect();
                let (min, maxsize) = (sizes.iter().min().unwrap(), sizes.iter().max().unwrap());
                proptest::prop_assert!(maxsize - min <= 1);
            }
        }
    }

    #[test]
    fn image_and_blank_items_are_one_slide_each_and_the_picture_names_the_deck() {
        let item = ItemId::from("i");
        let asset = AssetId::from("abc.png");
        let poster = compile_image(&item, "Plakat", &asset);
        assert_eq!(poster.slides.len(), 1);
        assert!(matches!(&poster.slides[0].body, SlideBody::Image { asset_id } if asset_id == &asset));
        let other = compile_image(&item, "Plakat", &AssetId::from("def.png"));
        assert_ne!(poster.deck_id, other.deck_id, "the screen must redraw when the picture changes");
        let blank = compile_blank(&item, "Homilia");
        assert!(matches!(&blank.slides[0].body, SlideBody::Text { lines, .. } if lines.is_empty()));
        assert_eq!((&poster.slides[0].part, &blank.slides[0].part), (&SlidePart::Picture, &SlidePart::Pause));
    }
}
