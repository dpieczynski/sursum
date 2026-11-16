// SPDX-License-Identifier: GPL-3.0-or-later
//! One OpenSong song file → [`Song`].

use serde::Deserialize;
use sha2::{Digest, Sha256};
use sursum_core::model::{Song, SongbookEntry};
use sursum_protocol::SongId;

use super::decode::decode_text;
use super::lyrics::{parse_lyrics, parse_presentation};
use super::warning::{Warning, WarningKind};

/// A song read from OpenSong, with the problems found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSong {
    pub song: Song,
    pub warnings: Vec<Warning>,
}

/// Why a file could not be imported at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SongError {
    #[error("not an OpenSong song (root element is not <song>)")]
    NotASong,
    #[error("invalid XML: {0}")]
    InvalidXml(String),
}

/// The plain-text elements of an OpenSong song. Elements with attributes or children
/// (`capo`, `backgrounds`, `style`, …) are not needed and are skipped by the deserializer.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SongXml {
    title: Option<String>,
    author: Option<String>,
    copyright: Option<String>,
    presentation: Option<String>,
    hymn_number: Option<String>,
    ccli: Option<String>,
    theme: Option<String>,
    alttheme: Option<String>,
    aka: Option<String>,
    key: Option<String>,
    key_line: Option<String>,
    tempo: Option<String>,
    time_sig: Option<String>,
    timesig: Option<String>,
    user1: Option<String>,
    user2: Option<String>,
    user3: Option<String>,
    lyrics: Option<String>,
}

/// A stable song id derived from the file's path inside `Songs/`, so importing the same library
/// again updates songs instead of duplicating them.
pub fn song_id_for_path(relative_path: &str) -> SongId {
    let digest = Sha256::digest(relative_path.as_bytes());
    let hex: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    SongId(format!("opensong-{hex}"))
}

/// Parses one song file. `relative_path` is the path inside `Songs/` (e.g. `Kolędy/Bóg się
/// rodzi`); `songbook` names the songbook that `hymn_number` refers to.
pub fn parse_song(bytes: &[u8], relative_path: &str, songbook: &str) -> Result<ImportedSong, SongError> {
    let (text, assumed_1250) = decode_text(bytes);
    if root_element(&text) != Some("song") {
        return Err(SongError::NotASong);
    }
    let xml: SongXml =
        quick_xml::de::from_str(&text).map_err(|error| SongError::InvalidXml(error.to_string()))?;
    let mut warnings = Vec::new();
    if assumed_1250 {
        warnings.push(Warning { line: None, kind: WarningKind::AssumedWindows1250 });
    }

    let file_name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let title = match non_empty(xml.title.as_deref()) {
        Some(title) => title.to_owned(),
        None => {
            warnings.push(Warning { line: None, kind: WarningKind::TitleFromFileName });
            file_name.to_owned()
        }
    };
    let mut titles = vec![title];
    titles.extend(non_empty(xml.aka.as_deref()).filter(|aka| *aka != titles[0]).map(str::to_owned));

    let lyrics = parse_lyrics(xml.lyrics.as_deref().unwrap_or_default());
    warnings.extend(lyrics.warnings.iter().cloned());
    if lyrics.verses.is_empty() {
        warnings.push(Warning { line: None, kind: WarningKind::NoLyrics });
    }
    let (verse_order, order_warnings) =
        parse_presentation(xml.presentation.as_deref().unwrap_or_default(), &lyrics.names);
    warnings.extend(order_warnings);

    let mut extra = std::collections::BTreeMap::new();
    let optional_fields = [
        ("key", &xml.key),
        ("key_line", &xml.key_line),
        ("tempo", &xml.tempo),
        ("time_sig", &xml.time_sig),
        ("timesig", &xml.timesig),
        ("user1", &xml.user1),
        ("user2", &xml.user2),
        ("user3", &xml.user3),
    ];
    for (name, value) in optional_fields {
        if let Some(value) = non_empty(value.as_deref()) {
            extra.insert(name.to_owned(), value.to_owned());
        }
    }
    if let Some((folder, _)) = relative_path.rsplit_once('/') {
        extra.insert("opensong_folder".to_owned(), folder.to_owned());
    }
    extra.insert("opensong_path".to_owned(), relative_path.to_owned());

    let themes = [xml.theme.as_deref(), xml.alttheme.as_deref()]
        .into_iter()
        .flatten()
        .flat_map(|themes| themes.split(';'))
        .filter_map(|theme| non_empty(Some(theme)).map(str::to_owned))
        .collect();

    let song = Song {
        id: song_id_for_path(relative_path),
        titles,
        authors: non_empty(xml.author.as_deref()).map(str::to_owned).into_iter().collect(),
        songbooks: non_empty(xml.hymn_number.as_deref())
            .map(|entry| SongbookEntry { songbook: songbook.to_owned(), entry: entry.to_owned() })
            .into_iter()
            .collect(),
        copyright: non_empty(xml.copyright.as_deref()).map(str::to_owned),
        ccli: non_empty(xml.ccli.as_deref()).map(str::to_owned),
        themes,
        verse_order,
        verses: lyrics.verses,
        extra,
    };
    Ok(ImportedSong { song, warnings })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Name of the first element, skipping the XML declaration, comments and doctype.
pub(super) fn root_element(text: &str) -> Option<&str> {
    let mut rest = text.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<?") {
            rest = after.split_once("?>")?.1;
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->")?.1;
        } else if let Some(after) = rest.strip_prefix("<!") {
            rest = after.split_once('>')?.1;
        } else {
            let name = rest.strip_prefix('<')?;
            let end = name.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(name.len());
            return Some(&name[..end]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Pieśń testowa</title>
  <author>Autor Testowy</author>
  <copyright>Domena publiczna</copyright>
  <presentation>V1 C V2 C</presentation>
  <hymn_number>123</hymn_number>
  <capo print="false"></capo>
  <aka>Inny tytuł</aka>
  <key>G</key>
  <time_sig>3/4</time_sig>
  <theme>Poranne; Uwielbienie</theme>
  <lyrics>[V1]
.G
 Zwrotka pierwsza
[C]
 Refren &amp; echo
[V2]
 Zwrotka druga</lyrics>
  <backgrounds resize="body" keep_aspect="false" link="false" background_as_text="false"/>
</song>"#;

    #[test]
    fn full_song() {
        let imported = parse_song(SONG.as_bytes(), "Poranne/Pieśń testowa", "Śpiewnik").unwrap();
        let song = imported.song;
        assert_eq!(song.titles, ["Pieśń testowa", "Inny tytuł"]);
        assert_eq!(song.authors, ["Autor Testowy"]);
        assert_eq!(song.songbooks, [SongbookEntry { songbook: "Śpiewnik".into(), entry: "123".into() }]);
        assert_eq!(song.copyright.as_deref(), Some("Domena publiczna"));
        assert_eq!(song.themes, ["Poranne", "Uwielbienie"]);
        let order: Vec<&str> = song.verse_order.iter().map(|name| name.0.as_str()).collect();
        assert_eq!(order, ["v1", "c", "v2", "c"]);
        assert_eq!(song.verses[1].blocks[0].lines[0].text, "Refren & echo");
        assert_eq!(song.extra.get("key").map(String::as_str), Some("G"));
        assert_eq!(song.extra.get("time_sig").map(String::as_str), Some("3/4"));
        assert_eq!(song.extra.get("opensong_folder").map(String::as_str), Some("Poranne"));
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    }

    #[test]
    fn same_path_gives_same_id() {
        assert_eq!(song_id_for_path("a/b"), song_id_for_path("a/b"));
        assert_ne!(song_id_for_path("a/b"), song_id_for_path("a/c"));
    }

    #[test]
    fn missing_title_uses_the_file_name() {
        let imported =
            parse_song(b"<song><lyrics>[V1]\n x</lyrics></song>", "Folder/Nazwa pliku", "S").unwrap();
        assert_eq!(imported.song.titles, ["Nazwa pliku"]);
        assert_eq!(imported.warnings[0].kind, WarningKind::TitleFromFileName);
    }

    #[test]
    fn other_documents_are_rejected() {
        assert_eq!(parse_song(b"<set name=\"x\"></set>", "x", "S"), Err(SongError::NotASong));
        assert!(matches!(parse_song(b"<song><title>x</song>", "x", "S"), Err(SongError::InvalidXml(_))));
        assert_eq!(parse_song(b"\x89PNG", "x", "S"), Err(SongError::NotASong));
    }

    #[test]
    fn carriage_return_entities_are_handled() {
        let imported =
            parse_song(b"<song><title>t</title><lyrics>[V1]&#13;\n a&#13;\n b</lyrics></song>", "t", "S")
                .unwrap();
        let lines: Vec<&str> = imported.song.verses[0].blocks[0].texts().collect();
        assert_eq!(lines, ["a", "b"]);
    }
}
