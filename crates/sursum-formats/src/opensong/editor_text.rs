// SPDX-License-Identifier: GPL-3.0-or-later
//! What the song editor accepts, turned into OpenSong markup for the parser.
//!
//! Two ways of typing are accepted, so that nobody has to learn OpenSong first:
//!
//! - **Plain text** (no `[section]` headers): blocks separated by an empty line are verses.
//!   A block starting with "Ref.", "Ref:", "Refren" or "R." is the refrain; a block that is
//!   only such a marker repeats the refrain there. "Most" or "Bridge" starts a bridge. A verse
//!   number at the start ("1. ", "2) ") is dropped (the slides number themselves).
//! - **OpenSong markup** (`[V1]`, `[C]`, `||`, `---`, `.` chord lines, `;` comments): lyric
//!   lines no longer need the leading space; it is added here.
//!
//! The import stays strict; this is only for text typed or pasted into the editor.

use super::markup::OPTIONAL_BREAK;

/// Markup for the parser and, for plain text with repeated refrains, the presentation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMarkup {
    pub markup: String,
    /// E.g. `V1 C V2 C`, when plain text repeats the refrain; `None` means "as written".
    pub order: Option<String>,
}

/// Converts editor text to OpenSong markup (see the module docs).
pub fn editor_markup(text: &str) -> EditorMarkup {
    let has_headers = text.lines().any(|line| line.trim_start().starts_with('['));
    if has_headers {
        EditorMarkup { markup: with_lyric_spaces(text), order: None }
    } else {
        plain_markup(text)
    }
}

/// Lyrics of a song in markup for the editor: without the leading spaces OpenSong needs, when
/// there are no chord lines (with chords, the columns must stay as they are).
pub fn markup_for_editor(markup: &str) -> String {
    if markup.lines().any(|line| line.starts_with('.')) {
        return markup.to_owned();
    }
    markup.lines().map(|line| line.strip_prefix(' ').unwrap_or(line)).collect::<Vec<_>>().join("\n")
}

/// Adds the leading space to every lyric line that lacks it.
fn with_lyric_spaces(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim_end();
            let marker = line.is_empty()
                || line == OPTIONAL_BREAK
                || line.starts_with(['[', '.', ';', ' ', '|'])
                || line.starts_with(|c: char| c.is_ascii_digit());
            if marker { line.to_owned() } else { format!(" {line}") }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a plain-text block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Verse,
    Refrain,
    Bridge,
}

/// Recognises "Ref.", "Ref:", "Refren", "R." and "Most"/"Bridge" at the start of a block and
/// returns the kind and the rest of the first line.
fn block_kind(first_line: &str) -> (Block, &str) {
    let trimmed = first_line.trim_start();
    let lower = trimmed.to_lowercase();
    let markers: [(&str, Block); 8] = [
        ("refren", Block::Refrain),
        ("ref.", Block::Refrain),
        ("ref:", Block::Refrain),
        ("ref", Block::Refrain),
        ("r.", Block::Refrain),
        ("r:", Block::Refrain),
        ("most", Block::Bridge),
        ("bridge", Block::Bridge),
    ];
    for (marker, kind) in markers {
        if lower.starts_with(marker) {
            // Markers are ASCII, so their length in the original is the same; `get` guards it.
            let Some(rest) = trimmed.get(marker.len()..) else { continue };
            let at_word_end =
                marker.ends_with(['.', ':']) || rest.is_empty() || rest.starts_with([' ', ':', '.']);
            if at_word_end {
                return (kind, rest.trim_start_matches([':', '.', ' ']));
            }
        }
    }
    (Block::Verse, trimmed)
}

/// Drops a verse number like "1. " or "2) " from the start of a line.
fn without_verse_number(line: &str) -> &str {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 2 {
        return line;
    }
    match line[digits..].strip_prefix(['.', ')']) {
        Some(rest) if rest.starts_with(' ') => rest.trim_start(),
        _ => line,
    }
}

fn plain_markup(text: &str) -> EditorMarkup {
    let normalized = text.replace("\r\n", "\n");
    let blocks = normalized.split("\n\n").map(str::trim).filter(|block| !block.is_empty());
    let mut out: Vec<String> = Vec::new();
    let mut order: Vec<String> = Vec::new();
    let (mut verses, mut refrains, mut bridges) = (0, 0, 0);
    let mut repeated = false;
    for block in blocks {
        let mut lines = block.lines();
        let first = lines.next().unwrap_or_default();
        let (kind, first_rest) = block_kind(first);
        let mut texts: Vec<&str> = std::iter::once(first_rest).chain(lines).map(str::trim).collect();
        texts.retain(|line| !line.is_empty());
        if kind == Block::Refrain && texts.is_empty() {
            // Only "Ref.": the refrain again.
            if refrains > 0 {
                order.push("C".to_owned());
                repeated = true;
            }
            continue;
        }
        let header = match kind {
            Block::Verse => {
                verses += 1;
                if let Some(first) = texts.first_mut() {
                    *first = without_verse_number(first);
                }
                format!("V{verses}")
            }
            Block::Refrain => {
                refrains += 1;
                if refrains == 1 { "C".to_owned() } else { format!("C{refrains}") }
            }
            Block::Bridge => {
                bridges += 1;
                if bridges == 1 { "B".to_owned() } else { format!("B{bridges}") }
            }
        };
        out.push(format!("[{header}]"));
        out.extend(texts.iter().map(|line| format!(" {line}")));
        out.push(String::new());
        order.push(header);
    }
    EditorMarkup { markup: out.join("\n").trim_end().to_owned(), order: repeated.then(|| order.join(" ")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic texts only (fixtures/LICENSES.md).

    #[test]
    fn plain_verses_are_separated_by_empty_lines() {
        let typed = "1. Pierwsza linia\ndruga linia\n\n2. Trzecia linia\nczwarta linia";
        let converted = editor_markup(typed);
        assert_eq!(
            converted.markup,
            "[V1]\n Pierwsza linia\n druga linia\n\n[V2]\n Trzecia linia\n czwarta linia"
        );
        assert_eq!(converted.order, None, "written order");
    }

    #[test]
    fn refrain_markers_define_and_repeat_the_refrain() {
        let typed = "Zwrotka pierwsza\n\nRef.: Refren pierwszy\nrefren drugi\n\nZwrotka druga\n\nRef.\n\nMost: przejście";
        let converted = editor_markup(typed);
        assert_eq!(
            converted.markup,
            "[V1]\n Zwrotka pierwsza\n\n[C]\n Refren pierwszy\n refren drugi\n\n[V2]\n Zwrotka druga\n\n[B]\n przejście"
        );
        assert_eq!(converted.order.as_deref(), Some("V1 C V2 C B"));
        for marker in ["Refren", "REF:", "R. ", "ref"] {
            assert_eq!(block_kind(&format!("{marker} tekst")).0, Block::Refrain, "{marker}");
        }
        assert_eq!(block_kind("Reformacja").0, Block::Verse, "a word that only starts like a marker");
        assert_eq!(block_kind("Mostem").0, Block::Verse);
    }

    #[test]
    fn markup_lines_get_their_space_and_markers_stay() {
        let typed = "[V1]\nbez spacji\n spacja już jest\n;komentarz\n.C   G\nz akordami||\n---\n1 numerowana\n[C]\nrefren";
        let converted = editor_markup(typed);
        assert_eq!(
            converted.markup,
            "[V1]\n bez spacji\n spacja już jest\n;komentarz\n.C   G\n z akordami||\n---\n1 numerowana\n[C]\n refren"
        );
    }

    #[test]
    fn the_editor_shows_lyrics_without_the_markup_spaces() {
        assert_eq!(markup_for_editor("[V1]\n jedna\n druga"), "[V1]\njedna\ndruga");
        let chords = "[V1]\n.C\n jedna";
        assert_eq!(markup_for_editor(chords), chords, "chord columns must not move");
        let round_trip = editor_markup(&markup_for_editor("[V1]\n jedna\n||\n[C]\n druga")).markup;
        assert_eq!(round_trip, "[V1]\n jedna\n||\n[C]\n druga");
    }

    #[test]
    fn crlf_and_extra_empty_lines_are_tolerated() {
        let converted = editor_markup("a\r\nb\r\n\r\n\r\n\r\nc\r\n");
        assert_eq!(converted.markup, "[V1]\n a\n b\n\n[V2]\n c");
    }
}
