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
    /// Label code for jump buttons, e.g. `v1`, `c`, or a custom section name; the remote
    /// localizes codes.
    pub label: String,
    pub body: SlideBody,
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
