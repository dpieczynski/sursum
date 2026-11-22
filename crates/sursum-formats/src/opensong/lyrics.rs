// SPDX-License-Identifier: GPL-3.0-or-later
//! The OpenSong lyrics markup (see `docs/opensong-format.md`):
//!
//! ```text
//! [V1]                 section header (V verse, C chorus, B bridge, P pre-chorus, T tag, 1-9)
//! .G       C           chord line, positions match the lyric line below
//!  Lyric line          lyric lines start with a space
//! ;comment             ignored
//!  first || second     `||` forces a new slide; `|` breaks the line
//! [V]
//! 1 verse one line     numbered lines: verse 1, 2… share the chord line above them
//! 2 verse two line
//! ```

use std::collections::{BTreeMap, BTreeSet};

use sursum_core::model::{BreakKind, ChordAt, Line, LinesBlock, Verse, VerseName};

use super::warning::{Warning, WarningKind};

/// The result of parsing lyrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLyrics {
    pub verses: Vec<Verse>,
    /// Section references as written in the file (upper case) → verse names, for resolving the
    /// presentation order.
    pub names: BTreeMap<String, VerseName>,
    pub warnings: Vec<Warning>,
}

/// Parses OpenSong lyrics.
pub fn parse_lyrics(text: &str) -> ParsedLyrics {
    let mut parser = Parser::default();
    for (index, raw) in text.lines().enumerate() {
        parser.line(raw.trim_end(), index + 1);
    }
    parser.finish()
}

/// Resolves a presentation order like `"V1 C V2 C"` against the parsed sections. Unknown
/// references are skipped with a warning.
pub fn parse_presentation(text: &str, names: &BTreeMap<String, VerseName>) -> (Vec<VerseName>, Vec<Warning>) {
    let mut order = Vec::new();
    let mut warnings = Vec::new();
    for token in text.split(|c: char| c.is_whitespace() || c == ',').filter(|token| !token.is_empty()) {
        let name = resolve(token, names);
        match name {
            Some(name) => order.push(name),
            None => warnings.push(Warning {
                line: None,
                kind: WarningKind::UnknownSectionInPresentation(token.to_owned()),
            }),
        }
    }
    (order, warnings)
}

/// Finds the verse a presentation token refers to: by the header as written, else by its
/// normalized name, treating `C1` and `C` (or `V1` and `V`) as the same section.
fn resolve(token: &str, names: &BTreeMap<String, VerseName>) -> Option<VerseName> {
    if let Some(name) = names.get(&token.to_uppercase()) {
        return Some(name.clone());
    }
    let (name, label) = section_name(token);
    if label.is_some() {
        return None;
    }
    let known = |candidate: &str| names.values().find(|known| known.0 == candidate).cloned();
    known(&name.0)
        .or_else(|| {
            name.0
                .strip_suffix('1')
                .filter(|base| !base.ends_with(|c: char| c.is_ascii_digit()))
                .and_then(known)
        })
        .or_else(|| {
            (!name.0.ends_with(|c: char| c.is_ascii_digit()))
                .then(|| format!("{}1", name.0))
                .and_then(|n| known(&n))
        })
}

/// Maps a section header to a verse name: `V1`→`v1`, `C`→`c`, `T`→`e` (tag), `3`→`v3`,
/// `V1A`→`v1a`. Anything else is a custom section: the header is kept as the label and the name
/// is a placeholder `o`, numbered once all sections are known (see [`Parser::finish`]).
fn section_name(header: &str) -> (VerseName, Option<String>) {
    let header = header.trim();
    let upper = header.to_uppercase();
    if !upper.is_empty() && upper.chars().all(|c| c.is_ascii_digit()) {
        return (VerseName::new(&format!("v{upper}")), None);
    }
    let mut chars = upper.chars();
    let kind = match chars.next() {
        Some('V') => "v",
        // `R` is the Polish "refren" (chorus), common in Polish libraries.
        Some('C' | 'R') => "c",
        Some('B') => "b",
        Some('P') => "p",
        Some('T' | 'E') => "e",
        Some('I') => "i",
        Some('O') => "o",
        _ => "",
    };
    let rest: String = chars.collect();
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &rest[digits.len()..];
    let standard = !kind.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && suffix.len() <= 1;
    if standard {
        (VerseName::new(&format!("{kind}{digits}{suffix}")), None)
    } else {
        (VerseName::new("o"), Some(header.to_owned()))
    }
}

/// One `[section]` while it is being read.
#[derive(Debug)]
struct Section {
    key: String,
    name: VerseName,
    label: Option<String>,
    blocks: Vec<LinesBlock>,
    /// Numbered lines (`1 …`, `2 …`) by verse number.
    numbered: BTreeMap<u32, Vec<LinesBlock>>,
}

impl Section {
    fn new(header: &str) -> Self {
        let (name, label) = section_name(header);
        Section {
            key: header.trim().to_uppercase(),
            name,
            label,
            blocks: vec![LinesBlock::default()],
            numbered: BTreeMap::new(),
        }
    }

    /// The letters of the section name, e.g. `v` for `[V]` or `[V1]`, used for numbered lines.
    fn kind(&self) -> String {
        self.name.0.chars().take_while(char::is_ascii_alphabetic).collect()
    }
}

#[derive(Debug, Default)]
struct Parser {
    sections: Vec<Section>,
    pending_chords: Option<Vec<ChordAt>>,
    warnings: Vec<Warning>,
}

impl Parser {
    fn line(&mut self, line: &str, number: usize) {
        let Some(first) = line.chars().next() else {
            self.pending_chords = None;
            return;
        };
        match first {
            '[' => self.start_section(line, number),
            '-' if line == super::markup::OPTIONAL_BREAK => self.optional_break(),
            ';' => {}
            '.' => self.pending_chords = Some(chord_positions(&line[1..])),
            digit if digit.is_ascii_digit() && !self.sections.is_empty() => {
                let verse = digit.to_digit(10).unwrap_or(1);
                self.lyric(&line[1..], number, Some(verse));
            }
            ' ' => {
                self.lyric(&line[1..], number, None);
                self.pending_chords = None;
            }
            other => {
                self.warnings
                    .push(Warning { line: Some(number), kind: WarningKind::UnexpectedLineStart(other) });
                self.lyric(line, number, None);
                self.pending_chords = None;
            }
        }
    }

    /// `---`: the verse may be split here if it is too long for one slide.
    fn optional_break(&mut self) {
        if let Some(section) = self.sections.last_mut()
            && let Some(block) = section.blocks.last_mut()
            && !block.lines.is_empty()
        {
            block.break_after = BreakKind::Optional;
            section.blocks.push(LinesBlock::default());
        }
    }

    fn start_section(&mut self, line: &str, number: usize) {
        let header = line.trim_start_matches('[').split(']').next().unwrap_or_default();
        let section = Section::new(header);
        if self.sections.iter().any(|existing| existing.key == section.key) {
            self.warnings.push(Warning {
                line: Some(number),
                kind: WarningKind::DuplicateSection(header.trim().to_owned()),
            });
        }
        self.sections.push(section);
        self.pending_chords = None;
    }

    /// Adds a lyric line (without its leading marker) to the current section, or to verse
    /// `numbered` of it.
    fn lyric(&mut self, text: &str, number: usize, numbered: Option<u32>) {
        if self.sections.is_empty() {
            self.warnings.push(Warning { line: Some(number), kind: WarningKind::LinesBeforeFirstSection });
            self.sections.push(Section::new("V1"));
        }
        let leading = text.chars().take_while(|c| c.is_whitespace()).count();
        let text = text.trim();
        let chords = self.pending_chords.clone().unwrap_or_default();
        let section = self.sections.last_mut().expect("a section exists");
        let blocks = match numbered {
            Some(verse) => section.numbered.entry(verse).or_insert_with(|| vec![LinesBlock::default()]),
            None => &mut section.blocks,
        };
        for (part_index, part) in text.split("||").enumerate() {
            if part_index > 0 {
                force_break(blocks);
            }
            let rows: Vec<&str> = part.split('|').map(str::trim).filter(|row| !row.is_empty()).collect();
            for (row_index, row) in rows.iter().enumerate() {
                // Chord positions refer to the first row only (a `|` split moves later text).
                let row_chords = if part_index == 0 && row_index == 0 {
                    shift_chords(&chords, leading, row)
                } else {
                    Vec::new()
                };
                current_block(blocks).lines.push(Line { text: (*row).to_owned(), chords: row_chords });
            }
        }
    }

    fn finish(mut self) -> ParsedLyrics {
        self.number_custom_sections();
        let mut verses: Vec<Verse> = Vec::new();
        let mut names = BTreeMap::new();
        for section in std::mem::take(&mut self.sections) {
            let kind = section.kind();
            let direct = finished_blocks(section.blocks);
            if !direct.is_empty() || section.numbered.is_empty() {
                names.insert(section.key.clone(), section.name.clone());
                add_verse(
                    &mut verses,
                    Verse { name: section.name.clone(), label: section.label.clone(), blocks: direct },
                );
            }
            for (number, blocks) in section.numbered {
                let name = VerseName::new(&format!("{kind}{number}"));
                names.insert(name.0.to_uppercase(), name.clone());
                add_verse(&mut verses, Verse { name, label: None, blocks: finished_blocks(blocks) });
            }
        }
        verses.retain(|verse| !verse.blocks.is_empty());
        ParsedLyrics { verses, names, warnings: self.warnings }
    }
}

impl Parser {
    /// Names custom sections `o1`, `o2`… in order of appearance: the same header twice is one
    /// section (merged like any repeated section), and a number an `[O1]` header already took
    /// is skipped, so `[Intro]` and `[O1]` stay two sections.
    fn number_custom_sections(&mut self) {
        let taken: BTreeSet<String> = self
            .sections
            .iter()
            .filter(|section| section.label.is_none())
            .map(|section| section.name.0.clone())
            .collect();
        let mut custom: BTreeMap<String, VerseName> = BTreeMap::new();
        let mut free = (1..).map(|number| format!("o{number}")).filter(|name| !taken.contains(name));
        for section in self.sections.iter_mut().filter(|section| section.label.is_some()) {
            let name = custom
                .entry(section.key.clone())
                .or_insert_with(|| VerseName::new(&free.next().expect("numbers never run out")));
            section.name = name.clone();
        }
    }
}

/// Appends a verse, merging it into an existing verse of the same name with a slide break.
fn add_verse(verses: &mut Vec<Verse>, verse: Verse) {
    match verses.iter_mut().find(|existing| existing.name == verse.name) {
        Some(existing) => {
            if let Some(last) = existing.blocks.last_mut() {
                last.break_after = BreakKind::Forced;
            }
            existing.blocks.extend(verse.blocks);
        }
        None => verses.push(verse),
    }
}

fn current_block(blocks: &mut Vec<LinesBlock>) -> &mut LinesBlock {
    if blocks.is_empty() {
        blocks.push(LinesBlock::default());
    }
    blocks.last_mut().expect("just ensured")
}

fn force_break(blocks: &mut Vec<LinesBlock>) {
    let last = current_block(blocks);
    if !last.lines.is_empty() {
        last.break_after = BreakKind::Forced;
        blocks.push(LinesBlock::default());
    }
}

/// Drops empty blocks; the last block of a verse never ends with a break.
fn finished_blocks(blocks: Vec<LinesBlock>) -> Vec<LinesBlock> {
    let mut blocks: Vec<LinesBlock> = blocks.into_iter().filter(|block| !block.lines.is_empty()).collect();
    if let Some(last) = blocks.last_mut() {
        last.break_after = BreakKind::None;
    }
    blocks
}

/// Chords of a chord line (without its `.`), with their character positions.
fn chord_positions(line: &str) -> Vec<ChordAt> {
    let mut chords = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (position, c) in line.chars().enumerate() {
        match (c.is_whitespace(), &mut current) {
            (false, Some((_, chord))) => chord.push(c),
            (false, None) => current = Some((position, c.to_string())),
            (true, Some(_)) => {
                chords.extend(current.take().map(|(position, chord)| ChordAt { position, chord }));
            }
            (true, None) => {}
        }
    }
    chords.extend(current.map(|(position, chord)| ChordAt { position, chord }));
    chords
}

/// Moves chord positions left by the trimmed indentation and drops chords past the row's end.
fn shift_chords(chords: &[ChordAt], leading: usize, row: &str) -> Vec<ChordAt> {
    let length = row.chars().count();
    chords
        .iter()
        .map(|chord| ChordAt { position: chord.position.saturating_sub(leading), chord: chord.chord.clone() })
        .filter(|chord| chord.position <= length)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(verse: &Verse) -> Vec<Vec<&str>> {
        verse.blocks.iter().map(|block| block.texts().collect()).collect()
    }

    fn names(parsed: &ParsedLyrics) -> Vec<&str> {
        parsed.verses.iter().map(|verse| verse.name.0.as_str()).collect()
    }

    #[test]
    fn sections_lines_and_comments() {
        let parsed = parse_lyrics("[V1]\n first line\n;a comment\n second line\n\n[C]\n refrain\n");
        assert_eq!(names(&parsed), ["v1", "c"]);
        assert_eq!(texts(&parsed.verses[0]), [["first line", "second line"]]);
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn chords_attach_to_the_next_line_with_positions() {
        let parsed = parse_lyrics("[V1]\n.G     C\n Amazing grace\n");
        let line = &parsed.verses[0].blocks[0].lines[0];
        assert_eq!(line.text, "Amazing grace");
        assert_eq!(
            line.chords,
            [ChordAt { position: 0, chord: "G".into() }, ChordAt { position: 6, chord: "C".into() }]
        );
    }

    #[test]
    fn double_bar_forces_a_new_slide_and_single_bar_breaks_lines() {
        let parsed = parse_lyrics("[V1]\n one | two||\n three\n");
        let verse = &parsed.verses[0];
        assert_eq!(texts(verse), [vec!["one", "two"], vec!["three"]]);
        assert_eq!(verse.blocks[0].break_after, BreakKind::Forced);
        assert_eq!(verse.blocks[1].break_after, BreakKind::None);
    }

    #[test]
    fn trailing_single_bar_adds_no_empty_line() {
        let parsed = parse_lyrics("[V1]\n one|\n two|\n");
        assert_eq!(texts(&parsed.verses[0]), [["one", "two"]]);
    }

    #[test]
    fn numbered_lines_become_separate_verses_sharing_chords() {
        let parsed = parse_lyrics("[V]\n.D\n1 first verse\n2 second verse\n[C]\n refrain\n");
        assert_eq!(names(&parsed), ["v1", "v2", "c"]);
        assert_eq!(parsed.verses[1].blocks[0].lines[0].chords[0].chord, "D");
        assert_eq!(parsed.names.get("V2"), Some(&VerseName::new("v2")));
    }

    #[test]
    fn header_variants_map_to_openlyrics_names() {
        let parsed = parse_lyrics("[v1]\n a\n[C2]\n b\n[T]\n c\n[V1A]\n d\n[3]\n e\n[Intro]\n f\n[R1]\n g\n");
        assert_eq!(names(&parsed), ["v1", "c2", "e", "v1a", "v3", "o1", "c1"]);
        assert_eq!(parsed.verses[5].label.as_deref(), Some("Intro"));
    }

    #[test]
    fn duplicate_sections_merge_with_a_break_and_warn() {
        let parsed = parse_lyrics("[C]\n one\n[C]\n two\n");
        assert_eq!(names(&parsed), ["c"]);
        assert_eq!(texts(&parsed.verses[0]), [["one"], ["two"]]);
        assert!(matches!(parsed.warnings[0].kind, WarningKind::DuplicateSection(_)));
    }

    #[test]
    fn a_repeated_custom_section_is_one_section_and_never_collides_with_o1() {
        let parsed = parse_lyrics("[Intro]\n a\n[V1]\n b\n[Intro]\n c\n[O1]\n d\n");
        assert_eq!(names(&parsed), ["o2", "v1", "o1"]);
        assert_eq!(texts(&parsed.verses[0]), [["a"], ["c"]]);
        assert_eq!(parsed.verses[0].label.as_deref(), Some("Intro"));
        assert!(matches!(&parsed.warnings[0].kind, WarningKind::DuplicateSection(name) if name == "Intro"));
        let (order, warnings) = parse_presentation("Intro V1 Intro O1", &parsed.names);
        let order: Vec<&str> = order.iter().map(|name| name.0.as_str()).collect();
        assert_eq!(order, ["o2", "v1", "o2", "o1"]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn lines_before_any_section_go_to_v1_with_a_warning() {
        let parsed = parse_lyrics(" lonely\n");
        assert_eq!(names(&parsed), ["v1"]);
        assert_eq!(parsed.warnings[0].kind, WarningKind::LinesBeforeFirstSection);
    }

    #[test]
    fn unexpected_line_start_is_kept_with_a_warning() {
        let parsed = parse_lyrics("[V1]\n/odd line\n");
        assert_eq!(texts(&parsed.verses[0]), [["/odd line"]]);
        assert_eq!(parsed.warnings[0].kind, WarningKind::UnexpectedLineStart('/'));
    }

    #[test]
    fn crlf_and_empty_sections() {
        let parsed = parse_lyrics("[V1]\r\n line\r\n[B]\r\n\r\n");
        assert_eq!(names(&parsed), ["v1"]);
    }

    #[test]
    fn polish_refrain_and_numbering_aliases_resolve() {
        let parsed = parse_lyrics("[V1]\n a\n[R1]\n b\n[C]\n c\n");
        let (order, warnings) = parse_presentation("1 C1 R1 V78", &parsed.names);
        let order: Vec<&str> = order.iter().map(|name| name.0.as_str()).collect();
        // "1" is V1; "C1" is the refrain [R1]; the typo "V78" is reported.
        assert_eq!(order, ["v1", "c1", "c1"]);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn chorus_with_and_without_number_are_the_same_reference() {
        let parsed = parse_lyrics("[V]\n a\n[C]\n b\n");
        let (order, warnings) = parse_presentation("V1 C1", &parsed.names);
        let order: Vec<&str> = order.iter().map(|name| name.0.as_str()).collect();
        assert_eq!(order, ["v", "c"]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn presentation_resolves_case_insensitively_and_skips_unknown() {
        let parsed = parse_lyrics("[V1]\n a\n[C]\n b\n[Intro]\n c\n");
        let (order, warnings) = parse_presentation("V1 c intro V9 C", &parsed.names);
        let order: Vec<&str> = order.iter().map(|name| name.0.as_str()).collect();
        assert_eq!(order, ["v1", "c", "o1", "c"]);
        assert_eq!(warnings.len(), 1);
    }
}
