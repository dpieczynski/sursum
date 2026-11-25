// SPDX-License-Identifier: GPL-3.0-or-later
//! One OpenSong set file (`Sets/<name>`) → a list of entries.
//!
//! Song entries only name the song (and sometimes its folder); finding it in the library is the
//! server's job, because only the server knows which songs exist. Custom slides and scripture
//! become plain text entries. Image groups are not supported yet and are reported.

use std::collections::BTreeMap;

use serde::Deserialize;
use sursum_core::model::{Song, VerseName};

use super::decode::decode_text;
use super::lyrics::parse_presentation;
use super::warning::Warning;

/// A set read from OpenSong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSet {
    /// The `name` attribute, else the file name.
    pub name: String,
    /// The file name inside `Sets/`; re-importing the same file replaces the set.
    pub file_name: String,
    pub entries: Vec<SetEntry>,
    /// Groups that were skipped, and why.
    pub skipped: Vec<SkippedGroup>,
}

/// A slide group of a set that was not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedGroup {
    /// The group's `name` attribute.
    pub group: String,
    pub reason: SkipReason,
}

/// Why a slide group was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// A custom or scripture group with no slides.
    NoSlides,
    /// A group type sursum does not import (e.g. `image`), as the file names it.
    Unsupported(String),
}

/// One slide group of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetEntry {
    /// A song from the library, by file name. `folder` is the path inside `Songs/` when the set
    /// file records it (`path` attribute); `presentation` overrides the verse order.
    Song { name: String, folder: Option<String>, presentation: Option<String> },
    /// Custom slides or scripture: each page is one slide, lines separated by `\n`.
    Text { title: String, pages: Vec<String> },
}

impl SetEntry {
    /// The path inside `Songs/` this song entry points to, if the set recorded a folder.
    pub fn song_path(&self) -> Option<String> {
        let SetEntry::Song { name, folder: Some(folder), .. } = self else { return None };
        let folder = folder.trim_matches('/');
        Some(if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") })
    }
}

/// Why a file is not a set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetError {
    #[error("not an OpenSong set (root element is not <set>)")]
    NotASet,
    #[error("invalid XML: {0}")]
    InvalidXml(String),
}

#[derive(Debug, Default, Deserialize)]
struct SetXml {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(default)]
    slide_groups: GroupsXml,
}

#[derive(Debug, Default, Deserialize)]
struct GroupsXml {
    #[serde(default, rename = "slide_group")]
    groups: Vec<GroupXml>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GroupXml {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@type")]
    kind: String,
    #[serde(rename = "@presentation")]
    presentation: Option<String>,
    #[serde(rename = "@path")]
    path: Option<String>,
    title: Option<String>,
    slides: SlidesXml,
}

#[derive(Debug, Default, Deserialize)]
struct SlidesXml {
    #[serde(default, rename = "slide")]
    slides: Vec<SlideXml>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SlideXml {
    body: Option<String>,
}

/// Parses one set file; `file_name` is its name inside `Sets/`.
pub fn parse_set(bytes: &[u8], file_name: &str) -> Result<ImportedSet, SetError> {
    let (text, _) = decode_text(bytes);
    if super::song::root_element(&text) != Some("set") {
        return Err(SetError::NotASet);
    }
    let xml: SetXml =
        quick_xml::de::from_str(&text).map_err(|error| SetError::InvalidXml(error.to_string()))?;
    let mut set = ImportedSet {
        name: non_empty(xml.name.as_deref()).unwrap_or(file_name).to_owned(),
        file_name: file_name.to_owned(),
        entries: Vec::new(),
        skipped: Vec::new(),
    };
    for group in xml.slide_groups.groups {
        match group.kind.as_str() {
            "song" => set.entries.push(SetEntry::Song {
                name: group.name.trim().to_owned(),
                folder: group.path,
                presentation: non_empty(group.presentation.as_deref()).map(str::to_owned),
            }),
            "custom" | "scripture" => {
                let title = non_empty(group.title.as_deref()).unwrap_or(&group.name).to_owned();
                let pages: Vec<String> = group
                    .slides
                    .slides
                    .iter()
                    .filter_map(|slide| non_empty(slide.body.as_deref()))
                    .map(|body| body.lines().map(str::trim_end).collect::<Vec<_>>().join("\n"))
                    .collect();
                if pages.is_empty() {
                    set.skipped.push(SkippedGroup { group: group.name, reason: SkipReason::NoSlides });
                } else {
                    set.entries.push(SetEntry::Text { title, pages });
                }
            }
            other => {
                let reason = SkipReason::Unsupported(other.to_owned());
                set.skipped.push(SkippedGroup { group: group.name.clone(), reason });
            }
        }
    }
    Ok(set)
}

/// Resolves a set's `presentation` override (e.g. `"V1 C"`) against a song from the library,
/// with the same rules as a song's own presentation (`C1` ≡ `C`, `1` ≡ `V1`, labels).
pub fn presentation_for(song: &Song, presentation: &str) -> (Vec<VerseName>, Vec<Warning>) {
    let mut names = BTreeMap::new();
    for verse in &song.verses {
        names.insert(verse.name.0.to_uppercase(), verse.name.clone());
        if let Some(label) = &verse.label {
            names.insert(label.to_uppercase(), verse.name.clone());
        }
    }
    parse_presentation(presentation, &names)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_core::model::Verse;

    #[test]
    fn minimal_set_without_name_uses_the_file_name() {
        let set = parse_set(b"<set><slide_groups/></set>", "Pusty").unwrap();
        assert_eq!((set.name.as_str(), set.entries.len()), ("Pusty", 0));
        assert_eq!(parse_set(b"<song/>", "x"), Err(SetError::NotASet));
        assert!(matches!(parse_set(b"<set><slide_groups>", "x"), Err(SetError::InvalidXml(_))));
    }

    #[test]
    fn song_paths_with_and_without_folders() {
        let song = |folder: Option<&str>| SetEntry::Song {
            name: "Pieśń".into(),
            folder: folder.map(str::to_owned),
            presentation: None,
        };
        assert_eq!(song(None).song_path(), None);
        assert_eq!(song(Some("")).song_path().as_deref(), Some("Pieśń"));
        assert_eq!(song(Some("Kolędy/")).song_path().as_deref(), Some("Kolędy/Pieśń"));
    }

    #[test]
    fn presentation_override_uses_opensong_rules() {
        let song = Song {
            id: sursum_protocol::SongId::from("s"),
            titles: vec!["Pieśń".into()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: vec![
                Verse::simple("v1", &["a"]),
                Verse::simple("c", &["b"]),
                Verse::simple("v2", &["c"]),
            ],
            extra: Default::default(),
        };
        let (order, warnings) = presentation_for(&song, "1 C1 V2 X");
        let names: Vec<&str> = order.iter().map(|name| name.0.as_str()).collect();
        assert_eq!(names, ["v1", "c", "v2"]);
        assert_eq!(warnings.len(), 1);
    }
}
