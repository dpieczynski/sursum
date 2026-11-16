// SPDX-License-Identifier: GPL-3.0-or-later
//! Sharing the song library between devices (two churches of one parish): export to a `.sursum`
//! file and merge such a file in. Nothing is ever overwritten blindly:
//!
//! - a song that does not exist here is added;
//! - identical content changes nothing;
//! - if one side's current version appears in the other side's history, the newer one wins;
//! - if both changed independently, the incoming version is stored as a separate copy.

use std::collections::HashMap;
use std::io::Write as _;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_core::model::Song;
use sursum_protocol::{MergeReport, SongId};

use crate::db::Database;

/// Version of the `.sursum` bundle format.
const FORMAT_VERSION: u32 = 1;

/// One song in a bundle: the current version and the hashes of every version it went through.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSong {
    pub song: Song,
    pub content_hash: String,
    pub history: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    songs: usize,
}

/// Writes all songs into a `.sursum` file (a ZIP with `manifest.json` and `songs.jsonl`).
pub fn export(database: &Database) -> anyhow::Result<Vec<u8>> {
    let songs = database.songs_with_history()?;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&serde_json::to_vec(&Manifest { format: FORMAT_VERSION, songs: songs.len() })?)?;
    zip.start_file("songs.jsonl", options)?;
    for (song, content_hash, history) in songs {
        serde_json::to_writer(&mut zip, &BundleSong { song, content_hash, history })?;
        zip.write_all(b"\n")?;
    }
    zip.finish()?;
    Ok(buffer.into_inner())
}

/// Reads the songs of a `.sursum` file.
///
/// Nothing in the file is trusted: sizes are capped (see [`crate::limits`]), every song's content
/// hash is computed again here, and history entries that are not SHA-256 hex are dropped.
pub fn read_bundle(bytes: &[u8]) -> anyhow::Result<Vec<BundleSong>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a .sursum file")?;
    let mut budget = crate::limits::Budget::default();
    let manifest = budget.read(archive.by_name("manifest.json").context("no manifest")?, "manifest.json")?;
    let manifest: Manifest = serde_json::from_slice(&manifest)?;
    anyhow::ensure!(manifest.format == FORMAT_VERSION, "unsupported bundle format {}", manifest.format);
    let lines = budget.read(archive.by_name("songs.jsonl").context("no songs")?, "songs.jsonl")?;
    let lines = std::str::from_utf8(&lines).context("songs.jsonl is not UTF-8")?;
    lines
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut song: BundleSong = serde_json::from_str(line)?;
            song.content_hash = crate::db::song_content_hash(&song.song)?;
            song.history.retain(|hash| is_sha256_hex(hash));
            Ok(song)
        })
        .collect()
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// What to do with one incoming song.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decision {
    Add,
    Unchanged,
    FastForward,
    KeepLocal,
    Conflict,
}

/// Compares an incoming song with the local one (`None` if there is none).
fn decide(incoming: &BundleSong, local: Option<&(String, Vec<String>)>) -> Decision {
    let Some((local_hash, local_history)) = local else { return Decision::Add };
    if &incoming.content_hash == local_hash {
        Decision::Unchanged
    } else if local_history.contains(&incoming.content_hash) {
        Decision::KeepLocal
    } else if incoming.history.contains(local_hash) {
        Decision::FastForward
    } else {
        Decision::Conflict
    }
}

/// Merges a bundle. With `dry_run` nothing is written.
pub fn merge(database: &mut Database, incoming: &[BundleSong], dry_run: bool) -> anyhow::Result<MergeReport> {
    let local: HashMap<SongId, (String, Vec<String>)> = database
        .songs_with_history()?
        .into_iter()
        .map(|(song, hash, history)| (song.id.clone(), (hash, history)))
        .collect();
    let mut report = MergeReport::default();
    for bundle_song in incoming {
        let decision = decide(bundle_song, local.get(&bundle_song.song.id));
        let to_store = match decision {
            Decision::Add => {
                report.added += 1;
                Some(bundle_song.song.clone())
            }
            Decision::FastForward => {
                report.updated += 1;
                Some(bundle_song.song.clone())
            }
            Decision::Unchanged => {
                report.unchanged += 1;
                None
            }
            Decision::KeepLocal => {
                report.kept_local += 1;
                None
            }
            Decision::Conflict => {
                let copy = conflict_copy(&bundle_song.song, &bundle_song.content_hash);
                if local.contains_key(&copy.id) {
                    // This exact conflict was merged before; its copy is already here.
                    report.unchanged += 1;
                    None
                } else {
                    report.conflicts.push(bundle_song.song.title().to_owned());
                    Some(copy)
                }
            }
        };
        if let Some(song) = to_store.filter(|_| !dry_run) {
            database.upsert_song(&song, "merge")?;
        }
    }
    Ok(report)
}

/// The incoming side of a conflict, stored next to the local song under its own id.
fn conflict_copy(song: &Song, content_hash: &str) -> Song {
    let mut copy = song.clone();
    let short: String = content_hash.chars().take(8).collect();
    copy.id = SongId(format!("{}-copy-{short}", song.id));
    if let Some(title) = copy.titles.first_mut() {
        title.push_str(" (kopia ze scalenia)");
    }
    copy
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_core::model::Verse;

    fn song(id: &str, text: &str) -> Song {
        Song {
            id: SongId::from(id),
            titles: vec![format!("Pieśń {id}")],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            verse_order: vec![],
            verses: vec![Verse::simple("v1", &[text])],
            extra: Default::default(),
        }
    }

    fn text_of(database: &Database, id: &str) -> String {
        let songs = database.load_songs().unwrap();
        let song = songs.iter().find(|song| song.id.as_str() == id).unwrap();
        song.verses[0].blocks[0].lines[0].text.clone()
    }

    #[test]
    fn two_devices_share_a_library_without_losing_anything() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        tulce.upsert_song(&song("a", "pierwsza wersja"), "editor").unwrap();
        tulce.upsert_song(&song("b", "tylko w Tulcach"), "editor").unwrap();

        // Gowarzewo receives everything.
        let bundle = read_bundle(&export(&tulce).unwrap()).unwrap();
        let report = merge(&mut gowarzewo, &bundle, false).unwrap();
        assert_eq!((report.added, report.conflicts.len()), (2, 0));

        // Gowarzewo fixes a typo; Tulce gets the newer version (fast-forward).
        gowarzewo.upsert_song(&song("a", "poprawiona wersja"), "editor").unwrap();
        let bundle = read_bundle(&export(&gowarzewo).unwrap()).unwrap();
        let dry = merge(&mut tulce, &bundle, true).unwrap();
        assert_eq!((dry.updated, dry.unchanged), (1, 1));
        assert_eq!(text_of(&tulce, "a"), "pierwsza wersja", "a dry run writes nothing");
        merge(&mut tulce, &bundle, false).unwrap();
        assert_eq!(text_of(&tulce, "a"), "poprawiona wersja");

        // An old bundle never overwrites a newer local song.
        let old = BundleSong {
            song: song("a", "pierwsza wersja"),
            content_hash: bundle[0].history[0].clone(),
            history: vec![bundle[0].history[0].clone()],
        };
        assert_eq!(merge(&mut tulce, &[old], false).unwrap().kept_local, 1);

        // Both change the same song: the incoming one becomes a copy.
        tulce.upsert_song(&song("b", "zmiana w Tulcach"), "editor").unwrap();
        gowarzewo.upsert_song(&song("b", "zmiana w Gowarzewie"), "editor").unwrap();
        let bundle = read_bundle(&export(&gowarzewo).unwrap()).unwrap();
        let report = merge(&mut tulce, &bundle, false).unwrap();
        assert_eq!(report.conflicts, ["Pieśń b"]);
        let again = merge(&mut tulce, &bundle, false).unwrap();
        assert!(again.conflicts.is_empty(), "the same conflict is reported once: {again:?}");
        assert_eq!(text_of(&tulce, "b"), "zmiana w Tulcach");
        assert_eq!(tulce.load_songs().unwrap().len(), 3);
    }

    #[test]
    fn other_files_are_rejected() {
        assert!(read_bundle(b"nope").is_err());
    }

    /// A ZIP with the given entries (name, content written `repeat` times).
    fn zip_of(entries: &[(&str, &[u8], usize)]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, content, repeat) in entries {
            zip.start_file(*name, options).unwrap();
            for _ in 0..*repeat {
                zip.write_all(content).unwrap();
            }
        }
        zip.finish().unwrap();
        buffer.into_inner()
    }

    #[test]
    fn hashes_from_the_file_are_not_trusted() {
        // A hand-made line with a non-hex "hash" and history (it once crashed the state actor).
        let mut line = serde_json::to_value(BundleSong {
            song: song("a", "z pliku"),
            content_hash: String::new(),
            history: vec![],
        })
        .unwrap();
        line["content_hash"] = "aąąąąąąąą".into();
        line["history"] = serde_json::json!(["nie-hash", "ą"]);
        let manifest = br#"{"format":1,"songs":1}"#;
        let jsonl = format!("{line}\n");
        let bytes = zip_of(&[("manifest.json", manifest, 1), ("songs.jsonl", jsonl.as_bytes(), 1)]);
        let bundle = read_bundle(&bytes).unwrap();
        assert_eq!(bundle[0].content_hash, crate::db::song_content_hash(&bundle[0].song).unwrap());
        assert!(bundle[0].history.is_empty());

        let mut database = Database::in_memory().unwrap();
        database.upsert_song(&song("a", "lokalnie"), "editor").unwrap();
        let report = merge(&mut database, &bundle, true).unwrap();
        assert_eq!(report.conflicts.len(), 1, "a conflict, not a crash");
    }

    #[test]
    fn a_zip_bomb_is_refused_without_unpacking_it() {
        let manifest = br#"{"format":1,"songs":1}"#;
        let zeros = vec![b' '; 1024 * 1024];
        let bytes = zip_of(&[("manifest.json", manifest, 1), ("songs.jsonl", &zeros, 70)]);
        assert!(bytes.len() < 1024 * 1024, "small on the wire");
        let error = read_bundle(&bytes).unwrap_err().to_string();
        assert!(error.contains("larger than"), "{error}");
    }
}
