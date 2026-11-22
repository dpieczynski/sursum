// SPDX-License-Identifier: GPL-3.0-or-later
//! Problems found while importing; collected into the import report.

use std::fmt;

/// One problem, with the 1-based lyrics line it refers to (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub line: Option<usize>,
    pub kind: WarningKind,
}

/// What kind of problem was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// Lines before the first `[section]` header; they were put into a verse named `v1`.
    LinesBeforeFirstSection,
    /// A lyric line that does not start with a space (OpenSong convention); kept as text.
    UnexpectedLineStart(char),
    /// The same section header appears twice; the second one was appended with a slide break.
    DuplicateSection(String),
    /// The presentation order names a section that does not exist; it was skipped.
    UnknownSectionInPresentation(String),
    /// The file is not valid UTF-8 and declares no other encoding; decoded as windows-1250.
    AssumedWindows1250,
    /// Bytes invalid in the file's encoding were replaced with U+FFFD.
    InvalidBytes,
    /// An element that should appear once appears again with other text; the first was used.
    RepeatedElement(String),
    /// The song has no title element; the file name was used.
    TitleFromFileName,
    /// The song contains no lyrics.
    NoLyrics,
}

impl fmt::Display for WarningKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WarningKind::LinesBeforeFirstSection => {
                write!(f, "lines before the first [section]; put into v1")
            }
            WarningKind::UnexpectedLineStart(c) => {
                write!(f, "lyric line starts with {c:?} instead of a space")
            }
            WarningKind::DuplicateSection(name) => {
                write!(f, "section [{name}] appears twice; merged with a slide break")
            }
            WarningKind::UnknownSectionInPresentation(name) => {
                write!(f, "presentation names unknown section {name:?}")
            }
            WarningKind::AssumedWindows1250 => {
                write!(f, "not valid UTF-8 and no other encoding declared; read as windows-1250")
            }
            WarningKind::InvalidBytes => {
                write!(f, "bytes invalid in the file's encoding; replaced with \u{fffd}")
            }
            WarningKind::RepeatedElement(name) => {
                write!(f, "<{name}> appears more than once; used the first")
            }
            WarningKind::TitleFromFileName => write!(f, "no <title>; used the file name"),
            WarningKind::NoLyrics => write!(f, "no lyrics"),
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}
