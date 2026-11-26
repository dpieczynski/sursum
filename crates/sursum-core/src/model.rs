// SPDX-License-Identifier: GPL-3.0-or-later
//! Songs and sets. The song model follows OpenLyrics 0.9 so it can be exported without loss.

use serde::{Deserialize, Serialize};
use sursum_protocol::{AssetId, ChurchId, ItemId, SetId, SongId};

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

/// A chord placed above a lyric line, at a character position (OpenSong `.` chord lines).
/// Stored so imports are lossless and exports can keep them; not shown on the screen.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChordAt {
    /// Position in characters (not bytes) within the line's text.
    pub position: usize,
    pub chord: String,
}

/// One line of lyrics. In JSON a line without chords is just a string.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "LineRepr", into = "LineRepr")]
pub struct Line {
    pub text: String,
    pub chords: Vec<ChordAt>,
}

impl Line {
    /// A line without chords.
    pub fn text(text: impl Into<String>) -> Self {
        Line { text: text.into(), chords: Vec::new() }
    }
}

/// Compact JSON form of [`Line`]: a plain string when there are no chords.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum LineRepr {
    Text(String),
    WithChords { text: String, chords: Vec<ChordAt> },
}

impl From<LineRepr> for Line {
    fn from(repr: LineRepr) -> Self {
        match repr {
            LineRepr::Text(text) => Line::text(text),
            LineRepr::WithChords { text, chords } => Line { text, chords },
        }
    }
}

impl From<Line> for LineRepr {
    fn from(line: Line) -> Self {
        if line.chords.is_empty() {
            LineRepr::Text(line.text)
        } else {
            LineRepr::WithChords { text: line.text, chords: line.chords }
        }
    }
}

/// A run of lines inside a verse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinesBlock {
    pub lines: Vec<Line>,
    #[serde(default)]
    pub break_after: BreakKind,
}

impl LinesBlock {
    /// The plain text of each line.
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|line| line.text.as_str())
    }
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
                lines: lines.iter().map(|line| Line::text(*line)).collect(),
                break_after: BreakKind::None,
            }],
        }
    }

    /// The custom label if any, otherwise the verse name (`v1`, `c`). The remote gets sections
    /// as [`sursum_protocol::SectionName`] (see `slides::section_name`) and words them itself.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccli: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<String>,
    /// Presentation order; empty means "verses in the order they are written".
    #[serde(default)]
    pub verse_order: Vec<VerseName>,
    pub verses: Vec<Verse>,
    /// Imported fields without a dedicated place (e.g. OpenSong `key`, `capo`, `user1`), kept so
    /// nothing is lost on import and can be exported again.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extra: std::collections::BTreeMap<String, String>,
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
    /// One uploaded picture, e.g. a poster with announcements.
    Image { title: String, asset_id: AssetId },
    /// A deliberate pause in the running order (e.g. during the homily): an empty slide, so
    /// "next" moves on naturally and nobody has to remember to press black.
    Blank { title: String },
}

impl ItemKind {
    /// The name shown in the set list (songs are named by the library, so `None`).
    pub fn own_title(&self) -> Option<&str> {
        match self {
            ItemKind::Song { .. } => None,
            ItemKind::Text { title, .. } | ItemKind::Image { title, .. } | ItemKind::Blank { title } => {
                Some(title)
            }
        }
    }
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
    /// When the service is, as the organist chose it: `YYYY-MM-DD`, optionally with `THH:MM`
    /// (see [`is_set_date`]). Lists of sets are ordered by it, never by a device clock, which a
    /// church device may not have right (ADR 0025).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The church the set is for (ADR 0025); `None` for a set made before the device's church
    /// was named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub church: Option<ChurchId>,
}

/// True for a set date as the remote's date field gives it: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`.
/// Such text sorts like the dates it names.
pub fn is_set_date(text: &str) -> bool {
    let digits =
        |part: &str, length: usize| part.len() == length && part.bytes().all(|byte| byte.is_ascii_digit());
    let (day, time) = match text.split_once('T') {
        Some((day, time)) => (day, Some(time)),
        None => (text, None),
    };
    let mut parts = day.split('-');
    let day_ok = matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(year), Some(month), Some(day), None) if digits(year, 4) && digits(month, 2) && digits(day, 2)
    );
    let time_ok = time.is_none_or(|time| {
        matches!(time.split_once(':'), Some((hours, minutes)) if digits(hours, 2) && digits(minutes, 2))
    });
    day_ok && time_ok
}

impl ServiceSet {
    /// Inserts an item right after `anchor`, or at the end if there is no anchor (or it is not
    /// in the set).
    pub fn insert_after(&mut self, anchor: Option<&ItemId>, item: SetItem) {
        let position =
            anchor.and_then(|anchor| self.position(anchor)).map_or(self.items.len(), |index| index + 1);
        self.items.insert(position, item);
    }

    /// Removes an item; returns where it was and the item itself (for undo).
    pub fn remove(&mut self, item_id: &ItemId) -> Option<(usize, SetItem)> {
        let index = self.position(item_id)?;
        Some((index, self.items.remove(index)))
    }

    /// Moves an item to `index` (clamped to the set's length). Returns false if it is not there.
    pub fn move_to(&mut self, item_id: &ItemId, index: usize) -> bool {
        let Some((_, item)) = self.remove(item_id) else { return false };
        let index = index.min(self.items.len());
        self.items.insert(index, item);
        true
    }

    /// Puts an item back at a former position (undo of [`ServiceSet::remove`]).
    pub fn restore(&mut self, index: usize, item: SetItem) {
        let index = index.min(self.items.len());
        self.items.insert(index, item);
    }

    /// Index of an item in the running order.
    pub fn position(&self, item_id: &ItemId) -> Option<usize> {
        self.items.iter().position(|item| &item.id == item_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_dates_are_days_with_an_optional_time() {
        for good in ["2026-11-22", "2026-11-22T10:00"] {
            assert!(is_set_date(good), "{good}");
        }
        for bad in ["", "22.11.2026", "2026-11-2", "2026-11-22T10", "2026-11-22 10:00", "2026-11-22-01"] {
            assert!(!is_set_date(bad), "{bad}");
        }
    }

    #[test]
    fn lines_without_chords_serialize_as_strings() {
        let block: LinesBlock = serde_json::from_str(
            r#"{"lines": ["a", {"text": "b", "chords": [{"position": 0, "chord": "G"}]}]}"#,
        )
        .unwrap();
        assert_eq!(block.lines[0], Line::text("a"));
        assert_eq!(block.lines[1].chords[0].chord, "G");
        let json = serde_json::to_string(&block.lines[0]).unwrap();
        assert_eq!(json, r#""a""#);
    }

    #[test]
    fn insert_after_anchor_or_at_the_end() {
        let item = |id: &str| SetItem {
            id: ItemId::from(id),
            kind: ItemKind::Text { title: id.into(), slides: vec![] },
        };
        let mut set = ServiceSet {
            id: SetId::from("s"),
            name: "s".into(),
            items: vec![item("a"), item("b")],
            date: None,
            church: None,
        };
        set.insert_after(Some(&ItemId::from("a")), item("x"));
        set.insert_after(None, item("y"));
        set.insert_after(Some(&ItemId::from("missing")), item("z"));
        let ids: Vec<&str> = set.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["a", "x", "b", "y", "z"]);
    }

    #[test]
    fn remove_move_and_restore() {
        let item = |id: &str| SetItem {
            id: ItemId::from(id),
            kind: ItemKind::Text { title: id.into(), slides: vec![] },
        };
        let mut set = ServiceSet {
            id: SetId::from("s"),
            name: "s".into(),
            items: vec![item("a"), item("b"), item("c")],
            date: None,
            church: None,
        };
        assert!(set.move_to(&ItemId::from("a"), 9));
        let ids = |set: &ServiceSet| set.items.iter().map(|item| item.id.0.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&set), ["b", "c", "a"]);
        let (index, removed) = set.remove(&ItemId::from("c")).unwrap();
        assert_eq!((index, ids(&set)), (1, vec!["b".to_owned(), "a".to_owned()]));
        set.restore(index, removed);
        assert_eq!(ids(&set), ["b", "c", "a"]);
        assert!(!set.move_to(&ItemId::from("missing"), 0));
    }

    proptest::proptest! {
        #[test]
        fn moves_keep_every_item_exactly_once(moves in proptest::collection::vec((0usize..6, 0usize..8), 0..20)) {
            let item = |id: usize| SetItem { id: ItemId(id.to_string()), kind: ItemKind::Text { title: String::new(), slides: vec![] } };
            let mut set = ServiceSet { id: SetId::from("s"), name: "s".into(), items: (0..6).map(item).collect(), date: None, church: None };
            for (which, to) in moves {
                set.move_to(&ItemId(which.to_string()), to);
            }
            let mut ids: Vec<String> = set.items.iter().map(|item| item.id.0.clone()).collect();
            ids.sort();
            proptest::prop_assert_eq!(ids, (0..6).map(|n| n.to_string()).collect::<Vec<_>>());
        }
    }

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
