// SPDX-License-Identifier: GPL-3.0-or-later
//! Sharing the song library between devices (two churches of one parish): export to a `.sursum`
//! file and merge such a file in. Nothing is ever overwritten blindly:
//!
//! - a song that does not exist here is added;
//! - identical content changes nothing;
//! - if one side's current version appears in the other side's history, the newer one wins;
//! - if both changed independently, the incoming version is stored as a separate copy;
//! - a deletion travels as a tombstone: it deletes the song here only if this device has the
//!   same or an older version; a version edited after the deletion wins and stays (or comes back).

use std::collections::HashMap;
use std::io::Write as _;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_core::model::Song;
use sursum_protocol::{MergeReport, SongId};

use crate::db::{Database, StoredSong};

/// Version of the `.sursum` bundle format.
const FORMAT_VERSION: u32 = 1;

/// One song in a bundle: the current version and the hashes of every version it went through.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSong {
    pub song: Song,
    pub content_hash: String,
    pub history: Vec<String>,
    /// A tombstone: the song was deleted on the exporting device.
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    songs: usize,
}

/// Writes all songs into a `.sursum` file (a ZIP with `manifest.json` and `songs.jsonl`).
pub fn export(database: &Database) -> anyhow::Result<Vec<u8>> {
    let songs = database.stored_songs()?;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&serde_json::to_vec(&Manifest { format: FORMAT_VERSION, songs: songs.len() })?)?;
    zip.start_file("songs.jsonl", options)?;
    for stored in songs {
        let StoredSong { song, content_hash, history, deleted } = stored;
        serde_json::to_writer(&mut zip, &BundleSong { song, content_hash, history, deleted })?;
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
    /// The other device deleted the version this device has (or an older one).
    Delete,
}

/// What this device has of a song.
struct Local {
    hash: String,
    history: Vec<String>,
    deleted: bool,
}

/// Compares an incoming song with the local one (`None` if there is none).
fn decide(incoming: &BundleSong, local: Option<&Local>) -> Decision {
    let known_there =
        |local: &Local| incoming.content_hash == local.hash || incoming.history.contains(&local.hash);
    let known_here =
        |local: &Local| incoming.content_hash == local.hash || local.history.contains(&incoming.content_hash);
    match (local, incoming.deleted) {
        (None, false) => Decision::Add,
        (None, true) => Decision::Unchanged,
        (Some(local), true) if local.deleted => Decision::Unchanged,
        (Some(local), true) if known_there(local) => Decision::Delete,
        // Edited here after (or apart from) the version deleted there: keep it.
        (Some(_), true) => Decision::KeepLocal,
        // Edited there after the very version this device deleted: the newer song comes back.
        // Anything else (an older or unrelated version from a stale file) stays deleted.
        (Some(local), false) if local.deleted => {
            if !known_here(local) && incoming.history.contains(&local.hash) {
                Decision::FastForward
            } else {
                Decision::KeepLocal
            }
        }
        (Some(local), false) if incoming.content_hash == local.hash => Decision::Unchanged,
        (Some(local), false) if local.history.contains(&incoming.content_hash) => Decision::KeepLocal,
        (Some(local), false) if incoming.history.contains(&local.hash) => Decision::FastForward,
        (Some(_), false) => Decision::Conflict,
    }
}

/// Merges a bundle. With `dry_run` nothing is written.
///
/// Everything happens in one transaction (all or nothing, and one disk sync on the SD card). The
/// histories in the file are remembered, so a stale file merged later is recognised as older.
pub fn merge(database: &mut Database, incoming: &[BundleSong], dry_run: bool) -> anyhow::Result<MergeReport> {
    if dry_run {
        return merge_in(database, incoming, true);
    }
    database.in_transaction(|database| merge_in(database, incoming, false))
}

fn merge_in(database: &mut Database, incoming: &[BundleSong], dry_run: bool) -> anyhow::Result<MergeReport> {
    let local: HashMap<SongId, Local> = database
        .stored_songs()?
        .into_iter()
        .map(|stored| {
            let local = Local { hash: stored.content_hash, history: stored.history, deleted: stored.deleted };
            (stored.song.id, local)
        })
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
            Decision::Delete => {
                report.deleted += 1;
                if !dry_run {
                    database.delete_song(bundle_song.song.id.as_str())?;
                }
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
        if !dry_run && decision != Decision::Conflict {
            // What the other device knew of this song (its own copy, not a conflict copy).
            let mut known = bundle_song.history.clone();
            known.push(bundle_song.content_hash.clone());
            database.remember_hashes(bundle_song.song.id.as_str(), &known)?;
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
            deleted: false,
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

    fn live_ids(database: &Database) -> Vec<String> {
        database.load_songs().unwrap().into_iter().map(|song| song.id.as_str().to_owned()).collect()
    }

    #[test]
    fn deletions_travel_but_never_take_a_newer_edit_with_them() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        tulce.upsert_song(&song("c", "duplikat"), "editor").unwrap();
        tulce.upsert_song(&song("d", "wspólna"), "editor").unwrap();
        merge(&mut gowarzewo, &read_bundle(&export(&tulce).unwrap()).unwrap(), false).unwrap();

        // Gowarzewo deletes both; Tulce edits "d" meanwhile.
        assert!(gowarzewo.delete_song("c").unwrap() && gowarzewo.delete_song("d").unwrap());
        assert!(!gowarzewo.delete_song("c").unwrap(), "already deleted");
        tulce.upsert_song(&song("d", "poprawiona w Tulcach"), "editor").unwrap();

        let tombstones = read_bundle(&export(&gowarzewo).unwrap()).unwrap();
        let report = merge(&mut tulce, &tombstones, false).unwrap();
        assert_eq!((report.deleted, report.kept_local), (1, 1), "{report:?}");
        assert_eq!(live_ids(&tulce), ["d"], "the edited song stays");
        assert_eq!(merge(&mut tulce, &tombstones, false).unwrap().deleted, 0, "nothing more to delete");

        // Back the other way: the newer "d" returns to Gowarzewo, the deleted "c" stays deleted.
        let report = merge(&mut gowarzewo, &read_bundle(&export(&tulce).unwrap()).unwrap(), false).unwrap();
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(live_ids(&gowarzewo), ["d"]);
        assert_eq!(text_of(&gowarzewo, "d"), "poprawiona w Tulcach");
    }

    /// Exports of Tulce after each of three edits of song "s" (v1, v2, v3).
    fn three_exports() -> Vec<Vec<BundleSong>> {
        let mut tulce = Database::in_memory().unwrap();
        ["v1", "v2", "v3"]
            .iter()
            .map(|text| {
                tulce.upsert_song(&song("s", text), "editor").unwrap();
                read_bundle(&export(&tulce).unwrap()).unwrap()
            })
            .collect()
    }

    #[test]
    fn a_stale_file_never_resurrects_a_deleted_song() {
        let files = three_exports();
        let mut gowarzewo = Database::in_memory().unwrap();
        merge(&mut gowarzewo, &files[1], false).unwrap();
        gowarzewo.delete_song("s").unwrap();
        let report = merge(&mut gowarzewo, &files[0], false).unwrap();
        assert_eq!(report.updated, 0, "{report:?}");
        assert!(live_ids(&gowarzewo).is_empty(), "v1 is older than the deleted v2");
        // A genuinely newer version (edited after v2) does come back.
        let report = merge(&mut gowarzewo, &files[2], false).unwrap();
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(text_of(&gowarzewo, "s"), "v3");
    }

    #[test]
    fn files_merged_out_of_order_make_no_conflict_copies() {
        let files = three_exports();
        let mut gowarzewo = Database::in_memory().unwrap();
        for file in [&files[0], &files[2], &files[1]] {
            let report = merge(&mut gowarzewo, file, false).unwrap();
            assert!(report.conflicts.is_empty(), "{report:?}");
        }
        assert_eq!(live_ids(&gowarzewo), ["s"]);
        assert_eq!(text_of(&gowarzewo, "s"), "v3");
    }

    #[test]
    fn a_failed_merge_stores_nothing() {
        let mut database = Database::in_memory().unwrap();
        let result: anyhow::Result<()> = database.in_transaction(|database| {
            database.upsert_song(&song("t", "zapisana?"), "merge")?;
            anyhow::bail!("disk full")
        });
        assert!(result.is_err());
        assert!(live_ids(&database).is_empty(), "all or nothing");
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
            deleted: false,
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
