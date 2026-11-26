// SPDX-License-Identifier: GPL-3.0-or-later
//! Imports the synthetic OpenSong library in `fixtures/opensong/Songs` (see `fixtures/LICENSES.md`).

use std::path::Path;

use sursum_formats::opensong::{SkipReason, WarningKind, read_library};

fn library() -> sursum_formats::opensong::LibraryImport {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/opensong/Songs");
    read_library(&dir, "Przykładowy").expect("fixtures readable")
}

fn song<'a>(import: &'a sursum_formats::opensong::LibraryImport, path: &str) -> &'a sursum_core::model::Song {
    &import
        .songs
        .iter()
        .find(|(relative, _)| relative == path)
        .unwrap_or_else(|| panic!("{path} not imported"))
        .1
        .song
}

fn order(song: &sursum_core::model::Song) -> Vec<&str> {
    song.verse_order.iter().map(|name| name.0.as_str()).collect()
}

#[test]
fn imports_every_song_reports_the_broken_one_and_skips_images() {
    let import = library();
    let paths: Vec<&str> = import.songs.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["Kolędy/Synthetic Numbered", "Synthetic Basic", "Synthetic CP1250"]);
    assert_eq!(import.failures.len(), 1);
    assert_eq!(import.failures[0].0, "Synthetic Broken");
}

#[test]
fn basic_song() {
    let import = library();
    let song = song(&import, "Synthetic Basic");
    assert_eq!(song.titles, ["Syntetyczna pieśń podstawowa"]);
    assert_eq!(order(song), ["v1", "c", "v2", "c"]);
    assert_eq!(song.songbooks[0].entry, "101");
    let verse = &song.verses[0];
    let lines: Vec<&str> = verse.blocks[0].texts().collect();
    assert_eq!(lines, ["Pierwsza linia zwrotki,", "druga linia zwrotki."]);
    assert_eq!(verse.blocks[0].lines[0].chords.len(), 2);
    assert_eq!(song.verses[1].blocks.len(), 2, "|| splits the refrain");
}

#[test]
fn numbered_lines_bom_crlf_and_polish_refrain() {
    let import = library();
    let song = song(&import, "Kolędy/Synthetic Numbered");
    let names: Vec<&str> = song.verses.iter().map(|verse| verse.name.0.as_str()).collect();
    assert_eq!(names, ["v1", "v2", "c"]);
    assert_eq!(order(song), ["v1", "c", "v2", "c"]);
    assert_eq!(song.verses[1].blocks[0].lines.len(), 2);
    assert_eq!(song.extra.get("opensong_folder").map(String::as_str), Some("Kolędy"));
}

#[test]
fn windows_1250_song_keeps_polish_letters() {
    let import = library();
    let song = song(&import, "Synthetic CP1250");
    let text: Vec<&str> = song.verses[0].blocks[0].texts().collect();
    assert_eq!(text, ["Zażółć gęślą jaźń –", "ĄĆĘŁŃÓŚŹŻ ąćęłńóśźż."]);
    assert!(song.verse_order.is_empty());
    let warnings = &import.songs.iter().find(|(path, _)| path == "Synthetic CP1250").unwrap().1.warnings;
    assert!(
        !warnings.iter().any(|warning| warning.kind == WarningKind::AssumedWindows1250),
        "encoding was declared"
    );
}

#[test]
fn set_file_lists_songs_texts_and_skipped_groups() {
    use sursum_formats::opensong::{SetEntry, read_sets};
    let mut import = sursum_formats::opensong::LibraryImport::default();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/opensong/Sets");
    read_sets(&mut import, &dir).unwrap();
    assert!(import.failures.is_empty(), "{:?}", import.failures);
    let set = &import.sets[0];
    assert_eq!((set.name.as_str(), set.file_name.as_str()), ("Syntetyczna niedziela", "Synthetic Sunday"));
    assert_eq!(set.entries.len(), 5);
    assert_eq!(
        set.entries[0],
        SetEntry::Song {
            name: "Synthetic Basic".into(),
            folder: Some(String::new()),
            presentation: Some("V1 C".into())
        }
    );
    assert_eq!(set.entries[0].song_path().as_deref(), Some("Synthetic Basic"));
    let SetEntry::Text { title, pages } = &set.entries[1] else { panic!("{:?}", set.entries[1]) };
    assert_eq!(title, "Ogłoszenia parafialne");
    assert_eq!(
        pages,
        &["Syntetyczne ogłoszenie pierwsze.\nDruga linia ogłoszenia.", "Syntetyczne ogłoszenie drugie."]
    );
    assert_eq!(set.entries[2].song_path(), None, "no path attribute: found by name later");
    assert!(matches!(&set.entries[3], SetEntry::Text { pages, .. } if pages.len() == 1));
    assert_eq!(set.entries[4].song_path().as_deref(), Some("Inne/Pieśń której nie ma"));
    assert_eq!(set.skipped.len(), 1);
    assert_eq!(set.skipped[0].reason, SkipReason::Unsupported("image".into()), "{:?}", set.skipped);
}
