// SPDX-License-Identifier: GPL-3.0-or-later
//! Search-key normalization that ignores Polish (and other) diacritics (ADR 0006).
//!
//! Unicode decomposition handles most accented letters (`ą` → `a` + ogonek), but some letters
//! have no decomposition at all, most importantly the Polish `ł`. Those are mapped explicitly.

use sursum_protocol::SongId;
use unicode_normalization::UnicodeNormalization;

use crate::model::Song;
use unicode_normalization::char::is_combining_mark;

/// Letters without a canonical decomposition that users expect to match their base letter.
const EXPLICIT_FOLDS: &[(char, &str)] = &[
    ('ł', "l"),
    ('đ', "d"),
    ('ø', "o"),
    ('ß', "ss"),
    ('æ', "ae"),
    ('œ', "oe"),
    ('ı', "i"),
    ('ħ', "h"),
    ('þ', "th"),
    ('ŀ', "l"),
];

/// Turns text into a search key: lowercase, without diacritics, words separated by single
/// spaces, punctuation removed.
///
/// `normalize("Kiedy ranne wstają zorze!")` gives `"kiedy ranne wstaja zorze"`, and
/// `normalize("Łaska")` gives `"laska"`.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.nfkd().filter(|ch| !is_combining_mark(*ch)).flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            match EXPLICIT_FOLDS.iter().find(|(from, _)| *from == ch) {
                Some((_, to)) => out.push_str(to),
                None => out.push(ch),
            }
        } else {
            pending_space = true;
        }
    }
    out
}

/// How well a song matches a query; higher is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    /// Every query word starts some word of the title or lyrics.
    AllWords = 1,
    /// The lyrics contain the query as a phrase.
    Phrase = 2,
    /// The first line starts with the query.
    FirstLinePrefix = 3,
    /// Every query word starts some word of a title.
    TitleWords = 4,
    /// A title starts with the query.
    TitlePrefix = 5,
    /// A songbook number starts with the query (while typing `12` → 12, 120…).
    NumberPrefix = 6,
    /// The query is exactly a songbook number.
    NumberExact = 7,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub song_id: SongId,
    pub rank: Rank,
}

/// Normalized text of one song, prepared once.
#[derive(Debug, Clone)]
struct Entry {
    song_id: SongId,
    /// Main title, for tie-breaking.
    title: String,
    titles: Vec<String>,
    numbers: Vec<String>,
    first_line: String,
    text: String,
}

/// An in-memory index over all songs (ADR 0006). A few thousand songs fit easily; a query is a
/// linear scan over prepared, normalized strings.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

impl SearchIndex {
    /// Builds the index.
    pub fn build<'a>(songs: impl IntoIterator<Item = &'a Song>) -> Self {
        SearchIndex { entries: songs.into_iter().map(entry).collect() }
    }

    /// Adds or replaces one song.
    pub fn upsert(&mut self, song: &Song) {
        self.remove(&song.id);
        self.entries.push(entry(song));
    }

    /// Removes one song.
    pub fn remove(&mut self, song_id: &SongId) {
        self.entries.retain(|entry| &entry.song_id != song_id);
    }

    /// Number of indexed songs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if no song is indexed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The best matches, best first. Ties are broken by shorter, then alphabetical, title.
    pub fn query(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let query = normalize(query);
        if query.is_empty() {
            return Vec::new();
        }
        let words: Vec<&str> = query.split(' ').collect();
        let mut hits: Vec<(Rank, &Entry)> = self
            .entries
            .iter()
            .filter_map(|entry| rank(entry, &query, &words).map(|rank| (rank, entry)))
            .collect();
        hits.sort_by(|(rank_a, a), (rank_b, b)| {
            rank_b.cmp(rank_a).then(a.title.len().cmp(&b.title.len())).then_with(|| a.title.cmp(&b.title))
        });
        hits.into_iter()
            .take(limit)
            .map(|(rank, entry)| SearchHit { song_id: entry.song_id.clone(), rank })
            .collect()
    }
}

fn entry(song: &Song) -> Entry {
    let lines: Vec<&str> =
        song.verses.iter().flat_map(|verse| &verse.blocks).flat_map(|block| block.texts()).collect();
    Entry {
        song_id: song.id.clone(),
        title: normalize(song.title()),
        titles: song.titles.iter().map(|title| normalize(title)).collect(),
        numbers: song.songbooks.iter().map(|entry| normalize(&entry.entry)).collect(),
        first_line: lines.first().map(|line| normalize(line)).unwrap_or_default(),
        text: normalize(&lines.join(" ")),
    }
}

/// The best rank of `entry` for a normalized query, or `None` if it does not match.
fn rank(entry: &Entry, query: &str, words: &[&str]) -> Option<Rank> {
    if entry.numbers.iter().any(|number| number == query) {
        return Some(Rank::NumberExact);
    }
    let numeric = query.starts_with(|c: char| c.is_ascii_digit());
    if numeric && entry.numbers.iter().any(|number| number.starts_with(query)) {
        return Some(Rank::NumberPrefix);
    }
    if entry.titles.iter().any(|title| title.starts_with(query)) {
        return Some(Rank::TitlePrefix);
    }
    if entry.titles.iter().any(|title| all_words_prefix(title, words)) {
        return Some(Rank::TitleWords);
    }
    if entry.first_line.starts_with(query) {
        return Some(Rank::FirstLinePrefix);
    }
    if entry.text.contains(query) {
        return Some(Rank::Phrase);
    }
    let everything = [entry.titles.join(" "), entry.text.clone()].join(" ");
    all_words_prefix(&everything, words).then_some(Rank::AllWords)
}

/// True if every query word is the beginning of some word in `text`.
fn all_words_prefix(text: &str, words: &[&str]) -> bool {
    words.iter().all(|word| text.split(' ').any(|candidate| candidate.starts_with(word)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_polish_letter_folds_to_its_base() {
        assert_eq!(normalize("ąćęłńóśźż"), "acelnoszz");
        assert_eq!(normalize("ĄĆĘŁŃÓŚŹŻ"), "acelnoszz");
    }

    #[test]
    fn punctuation_and_spacing_collapse() {
        assert_eq!(normalize("  Bądź pochwalon,  Boże wielki!  "), "badz pochwalon boze wielki");
    }

    #[test]
    fn other_letters_without_decomposition_fold_too() {
        assert_eq!(normalize("Straße Øre Æble"), "strasse ore aeble");
    }

    #[test]
    fn digits_are_kept() {
        assert_eq!(normalize("Nr 123a"), "nr 123a");
    }

    fn song(id: &str, title: &str, number: Option<&str>, lines: &[&str]) -> Song {
        use crate::model::{SongbookEntry, Verse};
        Song {
            id: SongId::from(id),
            titles: vec![title.to_owned()],
            authors: vec![],
            songbooks: number
                .map(|entry| SongbookEntry { songbook: "S".into(), entry: entry.into() })
                .into_iter()
                .collect(),
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: vec![Verse::simple("v1", lines)],
            extra: Default::default(),
        }
    }

    fn library() -> SearchIndex {
        let songs = [
            song(
                "ranne",
                "Kiedy ranne wstają zorze",
                Some("12"),
                &["Kiedy ranne wstają zorze,", "Tobie ziemia, Tobie morze"],
            ),
            song("laska", "Łaska Twoja", Some("120"), &["Twoja łaska nade mną", "jest jak poranne zorze"]),
            song("swiety", "Święty, Święty", Some("7"), &["Święty Pan Bóg zastępów"]),
            song("zorze", "Zorze wieczorne", None, &["Gdy zachodzą zorze"]),
        ];
        SearchIndex::build(&songs)
    }

    fn ids(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.song_id.as_str()).collect()
    }

    #[test]
    fn exact_number_comes_first_then_number_prefixes() {
        let hits = library().query("12", 10);
        assert_eq!(ids(&hits), ["ranne", "laska"]);
        assert_eq!(hits[0].rank, Rank::NumberExact);
        assert_eq!(hits[1].rank, Rank::NumberPrefix);
    }

    #[test]
    fn diacritics_and_l_stroke_are_ignored() {
        assert_eq!(ids(&library().query("laska", 10)), ["laska"]);
        assert_eq!(ids(&library().query("swiety", 10))[0], "swiety");
        assert_eq!(ids(&library().query("ŁASKA", 10)), ["laska"]);
    }

    #[test]
    fn title_prefix_beats_phrase_in_lyrics() {
        let hits = library().query("zorze", 10);
        assert_eq!(ids(&hits)[0], "zorze");
        assert_eq!(hits[0].rank, Rank::TitlePrefix);
        assert!(ids(&hits).contains(&"ranne"));
        assert!(ids(&hits).contains(&"laska"));
    }

    #[test]
    fn word_prefixes_anywhere_match() {
        let hits = library().query("pan zast", 10);
        assert_eq!(ids(&hits), ["swiety"]);
    }

    #[test]
    fn first_line_and_limit() {
        // Words of the title outrank the first line…
        assert_eq!(library().query("twoja lask", 10)[0].rank, Rank::TitleWords);
        // …but a query that only begins the first line ranks as such.
        let hits = library().query("swiety pan b", 10);
        assert_eq!(hits[0].rank, Rank::FirstLinePrefix);
        assert_eq!(library().query("o", 1).len(), 1);
        assert!(library().query("  ", 10).is_empty());
    }

    #[test]
    fn upsert_and_remove() {
        let mut index = library();
        index.upsert(&song("laska", "Inny tytuł", None, &["inny tekst"]));
        assert!(index.query("łaska", 10).is_empty());
        index.remove(&SongId::from("ranne"));
        assert_eq!(index.len(), 3);
    }

    /// Run with `cargo test --release -p sursum-core search_speed -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing, run manually in release mode"]
    fn search_speed_with_5000_songs() {
        let words =
            ["łaska", "zorze", "Pan", "chwała", "święty", "niebo", "ziemia", "serce", "Boże", "miłość"];
        let songs: Vec<Song> = (0..5000)
            .map(|n| {
                let lines: Vec<String> = (0..40)
                    .map(|line| {
                        (0..6)
                            .map(|w| words[(n * 7 + line * 3 + w) % words.len()])
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
                song(&format!("s{n}"), &format!("Pieśń {n}"), Some(&n.to_string()), &lines)
            })
            .collect();
        let index = SearchIndex::build(&songs);
        let start = std::time::Instant::now();
        let rounds = 50;
        for _ in 0..rounds {
            std::hint::black_box(index.query("chwała niebo", 20));
        }
        println!("5000 songs: {:?} per query", start.elapsed() / rounds);
    }

    #[test]
    fn empty_and_symbol_only_inputs_give_empty_keys() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize(" ,.;! "), "");
    }
}
