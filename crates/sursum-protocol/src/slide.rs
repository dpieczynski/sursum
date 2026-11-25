// SPDX-License-Identifier: GPL-3.0-or-later
//! Compiled slides as the screen draws them.

use serde::{Deserialize, Serialize};

use crate::ids::{AssetId, DeckId, ItemId, SlideKey};

/// All slides of one set item (e.g. a whole song), compiled by the server.
///
/// The screen fits one font size to the whole deck (`fit_group`), so the size does not jump
/// between verses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    pub deck_id: DeckId,
    /// Slides sharing a fit group share one font size.
    pub fit_group: String,
    pub item_id: ItemId,
    /// Title shown in the remote (never on the screen unless a theme asks for it).
    pub title: String,
    pub slides: Vec<Slide>,
}

/// One slide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slide {
    pub key: SlideKey,
    /// Which part of its item this slide shows. The remote names tiles and the live position
    /// from it ("Zwrotka 2 (2)", "Slajd 3"), so it never reads keys or section codes.
    pub part: SlidePart,
    pub body: SlideBody,
}

/// Which part of its item a slide shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlidePart {
    /// A slide of a song section. A section too long for one slide is split; `piece` counts
    /// its slides from 0, so the remote can tell the second slide of a verse from the first.
    Section { section: SectionName, piece: u32 },
    /// Page `number` (from 1) of a text item, e.g. an announcement.
    Page { number: u32 },
    /// The one slide of a picture item.
    Picture,
    /// An empty slide: a pause item, or the gap after an item's last slide.
    Pause,
}

/// A song section as the server read it from the markup (`[V2]`, `[C]`, `[Alleluja]`). The
/// remote words it in its language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionName {
    pub kind: SectionKind,
    /// The number after the kind (`2` in `V2`), if any.
    pub number: Option<u32>,
    /// A letter after the number (`a` in OpenSong's `V1a`), if any.
    #[serde(default)]
    pub suffix: Option<char>,
    /// A section with a name of its own (`[Alleluja]`): shown as written, instead of kind and
    /// number.
    #[serde(default)]
    pub label: Option<String>,
}

/// What kind of section a song section is (OpenLyrics names, OpenSong headers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// `v`, OpenSong `[V1]`.
    Verse,
    /// `c`, OpenSong `[C]` (or the Polish `[R]`).
    Chorus,
    /// `b`, OpenSong `[B]`.
    Bridge,
    /// `p`, OpenSong `[P]`.
    PreChorus,
    /// `e`, OpenSong `[T]` (tag) or `[E]`.
    Ending,
    /// `i`, OpenSong `[I]`.
    Intro,
    /// `o`, any other section, usually one with a name of its own.
    Other,
}

/// What a slide contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlideBody {
    /// Song lyrics; each entry is one line exactly as it should be displayed.
    Lyrics { lines: Vec<String> },
    /// Free text slide, e.g. an announcement.
    Text { title: Option<String>, lines: Vec<String> },
    /// An image, e.g. a poster: always whole, inside the content area, on black.
    Image { asset_id: AssetId },
}
