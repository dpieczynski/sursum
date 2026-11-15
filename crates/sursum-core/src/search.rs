// SPDX-License-Identifier: GPL-3.0-or-later
//! Search-key normalization that ignores Polish (and other) diacritics (ADR 0006).
//!
//! Unicode decomposition handles most accented letters (`ą` → `a` + ogonek), but some letters
//! have no decomposition at all, most importantly the Polish `ł`. Those are mapped explicitly.

use unicode_normalization::UnicodeNormalization;
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

    #[test]
    fn empty_and_symbol_only_inputs_give_empty_keys() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize(" ,.;! "), "");
    }
}
