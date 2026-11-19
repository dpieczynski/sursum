// SPDX-License-Identifier: GPL-3.0-or-later
//! Sharing songs between devices (the two churches of the parish, and the desktop program at
//! home): export to a `.sursum` file and merge such a file in (ADR 0025).
//!
//! A song keeps its id for good and gets versions; a file carries every song's heads. Merging
//! adds the versions this device did not know and keeps only what nothing replaces, so loading
//! any file again, in any direction, changes nothing, and no song is ever duplicated. When a song
//! was changed in two places, the version with more changes is shown on every device and the
//! other one stays with the song, waiting for review.

use std::io::Write as _;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_core::versions::{Body, Heads, Version, VersionId};
use sursum_protocol::{MergeReport, SongId};

use crate::db::{Database, song_from_body};

/// Version of the `.sursum` file format. Format 1 (songs with content hashes, before ADR 0025)
/// is refused, never converted.
const FORMAT_VERSION: u32 = 2;

/// Most heads one song may bring: more than a parish ever makes, fewer than a crafted file needs
/// to slow the merge down.
const MAX_HEADS: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    songs: usize,
}

/// One song in a file: its id and the heads of its history.
#[derive(Debug, Serialize, Deserialize)]
struct FileSong {
    id: String,
    heads: Vec<FileVersion>,
}

/// One version in a file. Its id is not written: the reader computes it from these fields, so
/// nothing in the file has to be trusted.
#[derive(Debug, Serialize, Deserialize)]
struct FileVersion {
    /// The song without its id, exactly as stored (JSON text); `null` for a deletion.
    body: Option<String>,
    /// Ids of every version this one replaces.
    ancestors: Vec<String>,
}

/// A song read from a file.
#[derive(Debug, Clone)]
pub struct IncomingSong {
    pub id: SongId,
    pub heads: Heads,
}

/// Writes every song, deleted ones included (a deletion travels too), into a `.sursum` file: a
/// ZIP with `manifest.json` and `songs.jsonl`.
pub fn export(database: &Database) -> anyhow::Result<Vec<u8>> {
    let songs = database.song_documents()?;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&serde_json::to_vec(&Manifest { format: FORMAT_VERSION, songs: songs.len() })?)?;
    zip.start_file("songs.jsonl", options)?;
    for (id, heads) in songs {
        let heads = heads.versions().iter().map(file_version).collect::<anyhow::Result<_>>()?;
        serde_json::to_writer(&mut zip, &FileSong { id: id.as_str().to_owned(), heads })?;
        zip.write_all(b"\n")?;
    }
    zip.finish()?;
    Ok(buffer.into_inner())
}

fn file_version(version: &Version) -> anyhow::Result<FileVersion> {
    let body = match version.body() {
        Body::Live(bytes) => Some(String::from_utf8(bytes.clone()).context("a stored song is not UTF-8")?),
        Body::Deleted => None,
    };
    let ancestors = version.ancestors().iter().map(|id| id.as_str().to_owned()).collect();
    Ok(FileVersion { body, ancestors })
}

/// Reads the songs of a `.sursum` file.
///
/// Nothing in the file is trusted: sizes are capped (see [`crate::limits`]), every version id is
/// computed here from its body and ancestors, ancestor ids must be well-formed, and every body
/// must be a song this program can read (a stored song it cannot read would stop the library
/// from loading).
pub fn read_bundle(bytes: &[u8]) -> anyhow::Result<Vec<IncomingSong>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a .sursum file")?;
    let mut budget = crate::limits::Budget::default();
    let manifest = budget.read(archive.by_name("manifest.json").context("no manifest")?, "manifest.json")?;
    let manifest: Manifest = serde_json::from_slice(&manifest)?;
    anyhow::ensure!(
        manifest.format == FORMAT_VERSION,
        "the file is in .sursum format {} (this program reads format {FORMAT_VERSION})",
        manifest.format
    );
    let lines = budget.read(archive.by_name("songs.jsonl").context("no songs")?, "songs.jsonl")?;
    let lines = std::str::from_utf8(&lines).context("songs.jsonl is not UTF-8")?;
    lines
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| incoming(serde_json::from_str(line)?))
        .collect()
}

fn incoming(song: FileSong) -> anyhow::Result<IncomingSong> {
    anyhow::ensure!(is_song_id(&song.id), "a song id in the file is malformed");
    anyhow::ensure!(
        (1..=MAX_HEADS).contains(&song.heads.len()),
        "song {} has {} versions in the file",
        song.id,
        song.heads.len()
    );
    let id = SongId(song.id);
    let versions = song
        .heads
        .into_iter()
        .map(|version| {
            let ancestors = version
                .ancestors
                .iter()
                .map(|ancestor| VersionId::parse(ancestor).context("an ancestor id in the file is malformed"))
                .collect::<anyhow::Result<_>>()?;
            let body = match version.body {
                Some(text) => {
                    song_from_body(&id, text.as_bytes()).with_context(|| format!("song {id} in the file"))?;
                    Body::Live(text.into_bytes())
                }
                None => Body::Deleted,
            };
            Ok(Version::new(body, ancestors))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(IncomingSong { id, heads: Heads::of(versions) })
}

/// Song ids are made by this program (`song-<uuid>`, `opensong-<hex>`, `demo-…`): letters,
/// digits, `-`, `_` and `.`, at most 128 characters.
fn is_song_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

/// Merges a file's songs. With `dry_run` nothing is written, and the report says exactly what a
/// real merge would do.
///
/// Everything happens in one transaction (all or nothing, and one disk sync on the SD card).
pub fn merge(
    database: &mut Database,
    incoming: &[IncomingSong],
    dry_run: bool,
) -> anyhow::Result<MergeReport> {
    if dry_run {
        return merge_in(database, incoming, true);
    }
    database.in_transaction(|database| merge_in(database, incoming, false))
}

fn merge_in(
    database: &mut Database,
    incoming: &[IncomingSong],
    dry_run: bool,
) -> anyhow::Result<MergeReport> {
    let mut report = MergeReport::default();
    for song in incoming {
        let before = database.song_heads(&song.id)?;
        let after = before.merge(&song.heads);
        match outcome(&before, &song.heads, &after) {
            Outcome::Unchanged => report.unchanged += 1,
            Outcome::KnownHere => report.kept_local += 1,
            Outcome::Added => report.added += 1,
            Outcome::Updated => report.updated += 1,
            Outcome::Deleted => report.deleted += 1,
            Outcome::ChangedInBoth => report.conflicts.push(title(&song.id, &after)?),
            Outcome::UnknownDeletion => {}
        }
        if !dry_run && after != before {
            database.store_song_heads(&song.id, &before, &after)?;
        }
    }
    Ok(report)
}

/// What merging did to one song, for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The file holds exactly what this device has.
    Unchanged,
    /// This device already knows everything the file holds of the song, or more.
    KnownHere,
    /// A song this device did not have.
    Added,
    /// The song now shows another version (or came back after a deletion).
    Updated,
    /// The song was deleted elsewhere in a version this device had.
    Deleted,
    /// Changed in two places: two different texts, one shown, the other waiting for review.
    ChangedInBoth,
    /// The deletion of a song this device never had: kept, so it can travel on.
    UnknownDeletion,
}

fn outcome(before: &Heads, incoming: &Heads, after: &Heads) -> Outcome {
    if after == before {
        return if incoming == before { Outcome::Unchanged } else { Outcome::KnownHere };
    }
    if before.is_empty() {
        return if after.is_live() { Outcome::Added } else { Outcome::UnknownDeletion };
    }
    if before.is_live() && !after.is_live() {
        return Outcome::Deleted;
    }
    if after.needs_review() {
        return Outcome::ChangedInBoth;
    }
    if before.current().map(Version::id) != after.current().map(Version::id) {
        return Outcome::Updated;
    }
    // Only a deletion that lost to a change arrived (or went away).
    Outcome::KnownHere
}

/// The title of the version a song shows, for the report.
fn title(id: &SongId, heads: &Heads) -> anyhow::Result<String> {
    let body = heads.current().and_then(Version::live_body).context("a song in review is live")?;
    Ok(song_from_body(id, body)?.title().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_core::model::{Song, Verse};

    use crate::db::{Base, SongSave};

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

    /// Saves like the editor does after loading the song: on top of what it shows now.
    fn edit(database: &mut Database, id: &str, text: &str) {
        let saved = database.save_song(&song(id, text), Base::Current).unwrap();
        assert!(matches!(saved, SongSave::Created(_) | SongSave::Changed(_)), "{saved:?}");
    }

    fn text_of(database: &Database, id: &str) -> String {
        let songs = database.load_songs().unwrap();
        let song = songs.iter().find(|song| song.id.as_str() == id).unwrap();
        song.verses[0].blocks[0].lines[0].text.clone()
    }

    fn live_ids(database: &Database) -> Vec<String> {
        database.load_songs().unwrap().into_iter().map(|song| song.id.as_str().to_owned()).collect()
    }

    fn file_of(database: &Database) -> Vec<IncomingSong> {
        read_bundle(&export(database).unwrap()).unwrap()
    }

    fn sync(from: &Database, to: &mut Database) -> MergeReport {
        merge(to, &file_of(from), false).unwrap()
    }

    #[test]
    fn two_devices_share_a_library_without_losing_anything() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        edit(&mut tulce, "a", "pierwsza wersja");
        edit(&mut tulce, "b", "tylko w Tulcach");
        let first_file = file_of(&tulce);

        let report = sync(&tulce, &mut gowarzewo);
        assert_eq!((report.added, report.conflicts.len()), (2, 0));

        // Gowarzewo fixes a typo; Tulce gets the newer version.
        edit(&mut gowarzewo, "a", "poprawiona wersja");
        let file = file_of(&gowarzewo);
        let dry = merge(&mut tulce, &file, true).unwrap();
        assert_eq!((dry.updated, dry.unchanged), (1, 1));
        assert_eq!(text_of(&tulce, "a"), "pierwsza wersja", "a dry run writes nothing");
        assert_eq!(merge(&mut tulce, &file, false).unwrap(), dry, "the dry run said what happens");
        assert_eq!(text_of(&tulce, "a"), "poprawiona wersja");

        // An old file changes nothing; loading the same file again changes nothing.
        let old = merge(&mut tulce, &first_file, false).unwrap();
        assert_eq!((old.kept_local, old.updated), (1, 0), "{old:?}");
        assert_eq!(merge(&mut tulce, &file, false).unwrap().unchanged, 2);
        assert_eq!(text_of(&tulce, "a"), "poprawiona wersja");
    }

    #[test]
    fn a_song_changed_in_both_churches_stays_one_song_with_the_same_text_everywhere() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        edit(&mut tulce, "x", "wspólna");
        sync(&tulce, &mut gowarzewo);
        edit(&mut tulce, "x", "poprawka z Tulec");
        edit(&mut tulce, "x", "druga poprawka z Tulec");
        edit(&mut gowarzewo, "x", "poprawka z Gowarzewa");

        let report = sync(&gowarzewo, &mut tulce);
        assert_eq!(report.conflicts, ["Pieśń x"], "{report:?}");
        sync(&tulce, &mut gowarzewo);
        for _ in 0..3 {
            sync(&gowarzewo, &mut tulce);
            sync(&tulce, &mut gowarzewo);
        }
        for church in [&tulce, &gowarzewo] {
            assert_eq!(live_ids(church), ["x"], "never a copy");
            assert_eq!(text_of(church, "x"), "druga poprawka z Tulec", "more changes win");
            let heads = church.song_heads(&SongId::from("x")).unwrap();
            assert!(heads.needs_review(), "the other text waits for review");
        }
        assert_eq!(
            tulce.song_heads(&SongId::from("x")).unwrap(),
            gowarzewo.song_heads(&SongId::from("x")).unwrap()
        );

        // An ordinary edit keeps the other text for review and travels like any correction.
        edit(&mut gowarzewo, "x", "po mszy");
        let report = sync(&gowarzewo, &mut tulce);
        assert_eq!(text_of(&tulce, "x"), "po mszy", "{report:?}");
        assert!(tulce.song_heads(&SongId::from("x")).unwrap().needs_review());
    }

    #[test]
    fn a_text_typed_back_wins_over_the_text_it_replaced() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        edit(&mut tulce, "s", "X");
        edit(&mut tulce, "s", "Y");
        sync(&tulce, &mut gowarzewo);
        edit(&mut tulce, "s", "X");
        let report = sync(&tulce, &mut gowarzewo);
        assert_eq!((report.updated, report.conflicts.len()), (1, 0), "{report:?}");
        assert_eq!(text_of(&gowarzewo, "s"), "X");
    }

    #[test]
    fn deletions_travel_but_never_take_a_change_made_elsewhere_with_them() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        edit(&mut tulce, "c", "duplikat");
        edit(&mut tulce, "d", "wspólna");
        sync(&tulce, &mut gowarzewo);

        // Gowarzewo deletes both; Tulce corrects "d" meanwhile.
        assert!(gowarzewo.delete_song("c").unwrap() && gowarzewo.delete_song("d").unwrap());
        edit(&mut tulce, "d", "poprawiona w Tulcach");

        let deletions = file_of(&gowarzewo);
        let report = merge(&mut tulce, &deletions, false).unwrap();
        assert_eq!((report.deleted, report.kept_local), (1, 1), "{report:?}");
        assert_eq!(live_ids(&tulce), ["d"], "the corrected song stays");
        assert_eq!(merge(&mut tulce, &deletions, false).unwrap().deleted, 0, "nothing more to delete");

        // Back the other way: the corrected "d" returns to Gowarzewo, "c" stays deleted.
        let report = sync(&tulce, &mut gowarzewo);
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(live_ids(&gowarzewo), ["d"]);
        assert_eq!(text_of(&gowarzewo, "d"), "poprawiona w Tulcach");
    }

    /// Files of Tulce after each of three edits of song "s" (v1, v2, v3).
    fn three_files() -> Vec<Vec<IncomingSong>> {
        let mut tulce = Database::in_memory().unwrap();
        ["v1", "v2", "v3"]
            .iter()
            .map(|text| {
                edit(&mut tulce, "s", text);
                file_of(&tulce)
            })
            .collect()
    }

    #[test]
    fn a_stale_file_never_brings_back_a_deleted_song() {
        let files = three_files();
        let mut gowarzewo = Database::in_memory().unwrap();
        merge(&mut gowarzewo, &files[1], false).unwrap();
        gowarzewo.delete_song("s").unwrap();
        let report = merge(&mut gowarzewo, &files[0], false).unwrap();
        assert_eq!(report.updated, 0, "{report:?}");
        assert!(live_ids(&gowarzewo).is_empty(), "v1 is older than the deleted v2");
        // A version made after v2 without knowing of the deletion does come back.
        let report = merge(&mut gowarzewo, &files[2], false).unwrap();
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(text_of(&gowarzewo, "s"), "v3");
    }

    #[test]
    fn files_merged_out_of_order_give_the_newest_version() {
        let files = three_files();
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
            database.save_song(&song("t", "zapisana?"), Base::Current)?;
            anyhow::bail!("disk full")
        });
        assert!(result.is_err());
        assert!(live_ids(&database).is_empty(), "all or nothing");
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

    fn file_with_line(line: &serde_json::Value) -> Vec<u8> {
        let manifest = br#"{"format":2,"songs":1}"#;
        let jsonl = format!("{line}\n");
        zip_of(&[("manifest.json", manifest, 1), ("songs.jsonl", jsonl.as_bytes(), 1)])
    }

    #[test]
    fn other_files_and_the_old_format_are_refused() {
        assert!(read_bundle(b"nope").is_err());
        let old = zip_of(&[("manifest.json", br#"{"format":1,"songs":0}"#, 1), ("songs.jsonl", b"", 1)]);
        let error = read_bundle(&old).unwrap_err().to_string();
        assert!(error.contains("format 1"), "{error}");
    }

    #[test]
    fn nothing_in_the_file_is_trusted() {
        let body = String::from_utf8(crate::db::song_body(&song("a", "z pliku")).unwrap()).unwrap();
        let good = serde_json::json!({ "id": "a", "heads": [{ "body": body, "ancestors": [] }] });
        let read = read_bundle(&file_with_line(&good)).unwrap();
        assert_eq!(read[0].heads.current().unwrap().id(), Version::first(body.clone().into_bytes()).id());

        let bad_ancestor =
            serde_json::json!({ "id": "a", "heads": [{ "body": body, "ancestors": ["nie-hash"] }] });
        let not_a_song = serde_json::json!({ "id": "a", "heads": [{ "body": "[1, 2]", "ancestors": [] }] });
        let bad_id = serde_json::json!({ "id": "a b/../c", "heads": [{ "body": body, "ancestors": [] }] });
        let no_heads = serde_json::json!({ "id": "a", "heads": [] });
        for line in [bad_ancestor, not_a_song, bad_id, no_heads] {
            assert!(read_bundle(&file_with_line(&line)).is_err(), "{line}");
        }
    }

    #[test]
    fn a_zip_bomb_is_refused_without_unpacking_it() {
        let manifest = br#"{"format":2,"songs":1}"#;
        let zeros = vec![b' '; 1024 * 1024];
        let bytes = zip_of(&[("manifest.json", manifest, 1), ("songs.jsonl", &zeros, 70)]);
        assert!(bytes.len() < 1024 * 1024, "small on the wire");
        let error = read_bundle(&bytes).unwrap_err().to_string();
        assert!(error.contains("larger than"), "{error}");
    }
}
