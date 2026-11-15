// SPDX-License-Identifier: GPL-3.0-or-later
//! Songs and sets. The song model follows OpenLyrics 0.9 so it can be exported without loss.

use serde::{Deserialize, Serialize};
use sursum_protocol::{ItemId, SetId, SongId};

/// Name of a verse in OpenLyrics style: `v1`, `c`, `c2`, `b`, `p`, `e`, `o1`…
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerseName(pub String);

impl VerseName {
    /// Creates a verse name; names are case-insensitive, so they are stored lowercase.
    pub fn new(name: &str) -> Self {
        VerseName(name.trim().to_lowercase())
    }
}

/// Where a block of lines may be split into separate slides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakKind {
    /// No break after this block within the verse (the last block of a verse).
    #[default]
    None,
    /// Always start a new slide after this block (OpenSong `||`).
    Forced,
    /// Start a new slide only if the verse is too long (OpenLyrics `break="optional"`).
    Optional,
}

/// A run of lines inside a verse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinesBlock {
    pub lines: Vec<String>,
    #[serde(default)]
    pub break_after: BreakKind,
}

/// One verse, chorus, bridge…
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Verse {
    pub name: VerseName,
    /// Optional custom label (for non-standard OpenSong section names).
    #[serde(default)]
    pub label: Option<String>,
    pub blocks: Vec<LinesBlock>,
}

impl Verse {
    /// A verse made of one block of lines.
    pub fn simple(name: &str, lines: &[&str]) -> Self {
        Verse {
            name: VerseName::new(name),
            label: None,
            blocks: vec![LinesBlock {
                lines: lines.iter().map(|line| (*line).to_owned()).collect(),
                break_after: BreakKind::None,
            }],
        }
    }

    /// Label code for jump buttons: the custom label if any, otherwise the verse name (`v1`,
    /// `c`). The remote turns codes into localized text; the server never sends UI text.
    pub fn display_label(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.name.0.clone())
    }
}

/// A song number in a songbook, e.g. `{ songbook: "Śpiewnik", entry: "123" }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SongbookEntry {
    pub songbook: String,
    pub entry: String,
}

/// A song.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    pub id: SongId,
    /// The first title is the main one; further titles are alternatives (OpenSong `aka`).
    pub titles: Vec<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub songbooks: Vec<SongbookEntry>,
    /// Presentation order; empty means "verses in the order they are written".
    #[serde(default)]
    pub verse_order: Vec<VerseName>,
    pub verses: Vec<Verse>,
}

impl Song {
    /// The main title (empty for untitled songs; the remote shows a placeholder).
    pub fn title(&self) -> &str {
        self.titles.first().map(String::as_str).unwrap_or_default()
    }

    /// Finds a verse by name.
    pub fn verse(&self, name: &VerseName) -> Option<&Verse> {
        self.verses.iter().find(|verse| &verse.name == name)
    }
}

/// What a set entry contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemKind {
    /// A song, optionally with a set-specific verse order.
    Song {
        song_id: SongId,
        #[serde(default)]
        order_override: Option<Vec<VerseName>>,
    },
    /// Custom text slides (announcements, prayers).
    Text { title: String, slides: Vec<String> },
}

/// One entry in a set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetItem {
    pub id: ItemId,
    pub kind: ItemKind,
}

/// A service's running order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSet {
    pub id: SetId,
    pub name: String,
    pub items: Vec<SetItem>,
}

impl ServiceSet {
    /// Index of an item in the running order.
    pub fn position(&self, item_id: &ItemId) -> Option<usize> {
        self.items.iter().position(|item| &item.id == item_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verse_names_are_lowercase() {
        assert_eq!(VerseName::new(" V1 ").0, "v1");
    }

    #[test]
    fn custom_label_wins_over_verse_name() {
        let mut verse = Verse::simple("o1", &["text"]);
        assert_eq!(verse.display_label(), "o1");
        verse.label = Some("Alleluja".to_owned());
        assert_eq!(verse.display_label(), "Alleluja");
    }
}
