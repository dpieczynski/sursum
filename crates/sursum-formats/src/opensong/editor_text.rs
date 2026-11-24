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
//!   lines no longer need the leading space; it is added here. A lyric line that would read as
//!   markup without it (" 2000 lat", " ...", " [bis]") keeps its space in the editor, so opening
//!   and saving a song never changes it.
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
/// there are no chord lines (with chords, the columns must stay as they are). A space is only
/// dropped where saving puts it back, so the round trip through the editor is lossless.
pub fn markup_for_editor(markup: &str) -> String {
    if markup.lines().any(|line| line.starts_with('.')) {
        return markup.to_owned();
    }
    markup
        .lines()
        .map(|line| match line.strip_prefix(' ') {
            Some(lyric) if gets_lyric_space(lyric.trim_end()) => lyric,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Adds the leading space to every lyric line that lacks it.
fn with_lyric_spaces(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim_end();
            if gets_lyric_space(line) { format!(" {line}") } else { line.to_owned() }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// True for an editor line that is lyrics, false for markup: empty lines, `---`, headers,
/// chord lines, comments, `|`/`||` breaks, numbered verse lines and lines that already have
/// their space.
fn gets_lyric_space(line: &str) -> bool {
    let marker = line.is_empty()
        || line == OPTIONAL_BREAK
        || line.starts_with(['[', '.', ';', ' ', '|'])
        || line.starts_with(|c: char| c.is_ascii_digit());
    !marker
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

/// Splits plain text into blocks at empty lines. A line of only spaces (or non-breaking spaces,
/// common in text pasted from a web page or a word processor) counts as empty.
fn plain_blocks(text: &str) -> Vec<String> {
    let mut blocks = vec![String::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            blocks.push(String::new());
        } else if let Some(block) = blocks.last_mut() {
            block.push_str(line);
            block.push('\n');
        }
    }
    blocks.into_iter().map(|block| block.trim().to_owned()).filter(|block| !block.is_empty()).collect()
}

fn plain_markup(text: &str) -> EditorMarkup {
    let blocks = plain_blocks(text);
    let mut out: Vec<String> = Vec::new();
    let mut order: Vec<String> = Vec::new();
    let (mut verses, mut refrains, mut bridges) = (0, 0, 0);
    let mut repeated = false;
    for block in &blocks {
        let mut lines = block.lines();
        let first = lines.next().unwrap_or_default();
        let (kind, first_rest) = block_kind(first);
        let mut texts: Vec<&str> = std::iter::once(first_rest).chain(lines).map(str::trim).collect();
        texts.retain(|line| !line.is_empty());
        if kind == Block::Refrain && texts.is_empty() {
            // Only "Ref.": the refrain here (also before the block that writes it out).
            order.push("C".to_owned());
            repeated = true;
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
    if refrains == 0 {
        // "Ref." with no refrain written anywhere: nothing to repeat.
        order.retain(|header| header != "C");
        repeated = false;
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
    fn lyric_lines_that_look_like_markup_keep_their_space() {
        // Each of these is a lyric line in OpenSong; without its space it would become a verse
        // number, a chord line, a comment, a section header or an optional break.
        let stored = "[V1]\n 2000 lat temu\n ...w ciszy\n ;uwaga\n [bis]\n ---\n | dalej\n zwykła linia";
        let shown = markup_for_editor(stored);
        assert_eq!(shown, "[V1]\n 2000 lat temu\n ...w ciszy\n ;uwaga\n [bis]\n ---\n | dalej\nzwykła linia");
        assert_eq!(editor_markup(&shown).markup, stored, "saving unchanged changes nothing");
    }

    #[test]
    fn opening_and_saving_never_changes_a_song() {
        // Every pair of a leading character class and a rest, as a lyric line among markup.
        let starts = ["", "1", "10 ", ".", "...", ";", "[", "[x]", "|", "||", "-", "---", "a", "Ą", "\u{a0}"];
        let rests = ["", "x", " tekst", "]", "---"];
        let mut lines = Vec::new();
        for start in starts {
            for rest in rests {
                let text = format!("{start}{rest}");
                if !text.trim().is_empty() {
                    lines.push(format!(" {}", text.trim_end()));
                }
            }
        }
        let stored = format!("[V1]\n{}\n||\n;komentarz\n---\n[C]\n refren", lines.join("\n"));
        assert_eq!(editor_markup(&markup_for_editor(&stored)).markup, stored);
    }

    #[test]
    fn a_refrain_marker_before_the_refrain_is_written_still_counts() {
        let typed = "Ref.\n\nZwrotka\n\nRef. Refren";
        assert_eq!(editor_markup(typed).order.as_deref(), Some("C V1 C"));
        assert_eq!(editor_markup("Zwrotka\n\nRef.").order, None, "no refrain written at all");
    }

    #[test]
    fn a_line_of_spaces_separates_verses_too() {
        let pasted = "pierwsza\nzwrotka\n   \ndruga\n\u{a0}\ntrzecia";
        assert_eq!(
            editor_markup(pasted).markup,
            "[V1]\n pierwsza\n zwrotka\n\n[V2]\n druga\n\n[V3]\n trzecia"
        );
    }

    #[test]
    fn crlf_and_extra_empty_lines_are_tolerated() {
        let converted = editor_markup("a\r\nb\r\n\r\n\r\n\r\nc\r\n");
        assert_eq!(converted.markup, "[V1]\n a\n b\n\n[V2]\n c");
    }
}
