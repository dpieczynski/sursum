// SPDX-License-Identifier: GPL-3.0-or-later
//! Turning file bytes into text: byte-order marks, declared encodings (e.g. ISO-8859-2 or
//! windows-1250 in older Polish libraries), and a windows-1250 fallback for undeclared files.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1250};

/// How a file's text was obtained, for the import report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoding {
    /// In the encoding of its byte-order mark or declaration, or valid UTF-8.
    Clean,
    /// Not valid UTF-8 and no other encoding declared: read as windows-1250.
    AssumedWindows1250,
    /// Bytes invalid in the file's own encoding were replaced with U+FFFD.
    Damaged,
}

/// Decodes an XML file. Returns the text and how it was obtained.
pub fn decode_text(bytes: &[u8]) -> (String, Decoding) {
    let checked = |had_errors: bool| if had_errors { Decoding::Damaged } else { Decoding::Clean };
    if let Some((encoding, bom_length)) = Encoding::for_bom(bytes) {
        let (text, had_errors) = encoding.decode_without_bom_handling(&bytes[bom_length..]);
        return (text.into_owned(), checked(had_errors));
    }
    if let Some(encoding) = declared_encoding(bytes) {
        let (text, _, had_errors) = encoding.decode(bytes);
        return (text.into_owned(), checked(had_errors));
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), Decoding::Clean),
        Err(_) => (WINDOWS_1250.decode(bytes).0.into_owned(), Decoding::AssumedWindows1250),
    }
}

/// The encoding named in `<?xml ... encoding="..."?>`, if any.
fn declared_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    // Only the declaration itself is ASCII; the rest may be in any encoding.
    let end = bytes.windows(2).take(200).position(|pair| pair == b"?>")?;
    let declaration = std::str::from_utf8(&bytes[..end]).ok()?.strip_prefix("<?xml")?;
    let after = declaration.split("encoding").nth(1)?.trim_start().strip_prefix('=')?.trim_start();
    let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let label = after[1..].split(quote).next()?;
    let encoding = Encoding::for_label(label.as_bytes())?;
    // A UTF-8 declaration on a file that is not UTF-8 is common in old libraries; let the
    // caller's fallback handle it.
    (encoding != UTF_8).then_some(encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_with_bom() {
        let bytes = "\u{feff}<song>Łąka</song>".as_bytes();
        assert_eq!(decode_text(bytes), ("<song>Łąka</song>".to_owned(), Decoding::Clean));
    }

    #[test]
    fn declared_windows_1250() {
        let mut bytes = br#"<?xml version="1.0" encoding="windows-1250"?><song>"#.to_vec();
        bytes.extend_from_slice(&WINDOWS_1250.encode("Łąka").0);
        bytes.extend_from_slice(b"</song>");
        assert!(decode_text(&bytes).0.ends_with("<song>Łąka</song>"));
    }

    #[test]
    fn declaration_is_found_even_when_the_body_is_not_utf8() {
        let mut bytes = br#"<?xml version="1.0" encoding="windows-1250"?><song><title>"#.to_vec();
        bytes.extend_from_slice(&WINDOWS_1250.encode("Pieśń").0);
        assert_eq!(decode_text(&bytes).1, Decoding::Clean);
    }

    #[test]
    fn undeclared_non_utf8_falls_back_to_windows_1250() {
        let mut bytes = b"<song>".to_vec();
        bytes.extend_from_slice(&WINDOWS_1250.encode("żółw").0);
        bytes.extend_from_slice(b"</song>");
        assert_eq!(decode_text(&bytes), ("<song>żółw</song>".to_owned(), Decoding::AssumedWindows1250));
    }

    #[test]
    fn invalid_bytes_in_a_utf16_file_are_reported() {
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend("<song>".encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend([0x00, 0xd8]); // a lone surrogate
        assert_eq!(decode_text(&bytes).1, Decoding::Damaged);
    }
}
