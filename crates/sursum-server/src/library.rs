// SPDX-License-Identifier: GPL-3.0-or-later
//! Sharing songs, sets and churches between devices (the two churches of the parish, and the
//! desktop program at home): export to a `.sursum` file and merge such a file in (ADR 0025).
//!
//! A song or a set keeps its id for good and gets versions; a file carries every document's
//! heads, and the pictures the sets use. Merging adds the versions this device did not know and
//! keeps only what nothing replaces, so loading any file again, in any direction, changes
//! nothing, and nothing is ever duplicated. When a document was changed in two places, the
//! version with more changes is shown on every device and the other one stays with it, waiting
//! for review.

use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};
use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sursum_core::model::{ItemKind, ServiceSet};
use sursum_core::versions::{Body, Heads, Version, VersionId};
use sursum_protocol::{AssetId, ChurchId, MergeReport, SetId, SongId};

use crate::db::{Database, church_name, set_from_body, song_from_body};

/// Version of the `.sursum` file format. Format 1 (songs with content hashes, before ADR 0025)
/// is refused, never converted.
const FORMAT_VERSION: u32 = 2;

/// Most heads one song may bring: more than a parish ever makes, fewer than a crafted file needs
/// to slow the merge down.
const MAX_HEADS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    songs: usize,
    #[serde(default)]
    churches: usize,
    #[serde(default)]
    sets: usize,
    #[serde(default)]
    pictures: usize,
}

/// Where a file keeps the pictures its sets use, each under its id.
const PICTURES: &str = "assets/";

/// One document (a song, a church or a set) in a file: its id and the heads of its history.
#[derive(Debug, Serialize, Deserialize)]
struct FileDocument {
    id: String,
    heads: Vec<FileVersion>,
}

/// One version in a file. Its id is not written: the reader computes it from these fields, so
/// nothing in the file has to be trusted.
#[derive(Debug, Serialize, Deserialize)]
struct FileVersion {
    /// The document without its id, exactly as stored (JSON text); `null` for a deletion.
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

/// A church read from a file.
#[derive(Debug, Clone)]
pub struct IncomingChurch {
    pub id: ChurchId,
    pub heads: Heads,
}

/// A set read from a file.
#[derive(Debug, Clone)]
pub struct IncomingSet {
    pub id: SetId,
    pub heads: Heads,
}

/// Everything a file holds (the pictures by id; their bytes stay in the file until stored).
#[derive(Debug, Clone, Default)]
pub struct Incoming {
    pub songs: Vec<IncomingSong>,
    pub churches: Vec<IncomingChurch>,
    pub sets: Vec<IncomingSet>,
    pub pictures: Vec<AssetId>,
}

/// What a library file will hold, gathered by the state actor in one go. The documents are small;
/// the pictures are read from the picture store later, outside the actor, so a large file never
/// holds up navigation.
#[derive(Debug, Clone)]
pub struct Export {
    manifest: Manifest,
    songs: Vec<u8>,
    churches: Vec<u8>,
    sets: Vec<u8>,
    pictures: Vec<AssetId>,
}

/// Every song, church and set, deleted ones included (a deletion travels too), and every
/// picture a live version of a set uses.
pub fn collect(database: &Database) -> anyhow::Result<Export> {
    let songs = database.song_documents()?;
    let churches = database.church_documents()?;
    let sets = database.set_documents()?;
    let mut pictures = BTreeSet::new();
    for (id, heads) in &sets {
        for body in heads.versions().iter().filter_map(Version::live_body) {
            pictures.extend(set_from_body(id, body)?.items.into_iter().filter_map(|item| match item.kind {
                ItemKind::Image { asset_id, .. } => Some(asset_id),
                _ => None,
            }));
        }
    }
    let lines = |documents: Vec<(String, Heads)>| -> anyhow::Result<Vec<u8>> {
        let mut out = Vec::new();
        for (id, heads) in documents {
            write_document(&mut out, &id, &heads)?;
        }
        Ok(out)
    };
    let manifest = Manifest {
        format: FORMAT_VERSION,
        songs: songs.len(),
        churches: churches.len(),
        sets: sets.len(),
        pictures: pictures.len(),
    };
    Ok(Export {
        manifest,
        songs: lines(songs.into_iter().map(|(id, heads)| (id.0, heads)).collect())?,
        churches: lines(churches.into_iter().map(|(id, heads)| (id.0, heads)).collect())?,
        sets: lines(sets.into_iter().map(|(id, heads)| (id.0, heads)).collect())?,
        pictures: pictures.into_iter().collect(),
    })
}

/// Writes a `.sursum` file: a ZIP with `manifest.json`, `songs.jsonl`, `churches.jsonl`,
/// `sets.jsonl` and `assets/<id>` for each picture (stored, not compressed again). A picture
/// missing from this device's store is left out; the device that loads the file shows nothing for
/// it, as here.
pub fn write(export: &Export, data_dir: &Path, out: impl Write + Seek) -> anyhow::Result<()> {
    let mut zip = zip::ZipWriter::new(out);
    let deflated =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("manifest.json", deflated)?;
    zip.write_all(&serde_json::to_vec(&export.manifest)?)?;
    for (name, lines) in
        [("songs.jsonl", &export.songs), ("churches.jsonl", &export.churches), ("sets.jsonl", &export.sets)]
    {
        zip.start_file(name, deflated)?;
        zip.write_all(lines)?;
    }
    for id in &export.pictures {
        let Some((path, _)) = crate::assets::locate(data_dir, id.as_str()) else {
            tracing::warn!(picture = id.as_str(), "a set uses a picture this device does not have");
            continue;
        };
        zip.start_file(format!("{PICTURES}{}", id.as_str()), stored)?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut zip)?;
    }
    zip.finish()?;
    Ok(())
}

/// The whole file in memory (tests, small libraries).
pub fn export(database: &Database, data_dir: &Path) -> anyhow::Result<Vec<u8>> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    write(&collect(database)?, data_dir, &mut buffer)?;
    Ok(buffer.into_inner())
}

fn write_document(out: &mut impl Write, id: &str, heads: &Heads) -> anyhow::Result<()> {
    let heads = heads.versions().iter().map(file_version).collect::<anyhow::Result<_>>()?;
    serde_json::to_writer(&mut *out, &FileDocument { id: id.to_owned(), heads })?;
    out.write_all(b"\n")?;
    Ok(())
}

fn file_version(version: &Version) -> anyhow::Result<FileVersion> {
    let body = match version.body() {
        Body::Live(bytes) => Some(String::from_utf8(bytes.clone()).context("a stored body is not UTF-8")?),
        Body::Deleted => None,
    };
    let ancestors = version.ancestors().iter().map(|id| id.as_str().to_owned()).collect();
    Ok(FileVersion { body, ancestors })
}

/// Reads the songs, churches and sets of a `.sursum` file, and which pictures it carries.
///
/// Nothing in the file is trusted: sizes are capped (see [`crate::limits`]), every version id is
/// computed here from its body and ancestors, ancestor ids must be well-formed, and every body
/// must be a document this program can read (a stored song it cannot read would stop the library
/// from loading). Pictures are checked when they are stored ([`store_pictures`]).
pub fn read_bundle(reader: impl Read + Seek) -> anyhow::Result<Incoming> {
    let mut archive = zip::ZipArchive::new(reader).context("not a .sursum file")?;
    anyhow::ensure!(archive.len() <= crate::limits::ARCHIVE_ENTRIES, "the file has too many entries");
    let mut budget = crate::limits::Budget::default();
    let manifest = budget.read(archive.by_name("manifest.json").context("no manifest")?, "manifest.json")?;
    let manifest: Manifest = serde_json::from_slice(&manifest)?;
    anyhow::ensure!(
        manifest.format == FORMAT_VERSION,
        "the file is in .sursum format {} (this program reads format {FORMAT_VERSION})",
        manifest.format
    );
    let songs = read_lines(&mut archive, &mut budget, "songs.jsonl")?
        .into_iter()
        .map(|document| {
            let id = SongId(document.id.clone());
            let heads = heads_of(document, |body| song_from_body(&id, body).map(drop))?;
            Ok(IncomingSong { id, heads })
        })
        .collect::<anyhow::Result<_>>()?;
    let churches = match archive.index_for_name("churches.jsonl") {
        None => Vec::new(),
        Some(_) => read_lines(&mut archive, &mut budget, "churches.jsonl")?
            .into_iter()
            .map(|document| {
                let id = ChurchId(document.id.clone());
                Ok(IncomingChurch { id, heads: heads_of(document, |body| church_name(body).map(drop))? })
            })
            .collect::<anyhow::Result<_>>()?,
    };
    let sets = match archive.index_for_name("sets.jsonl") {
        None => Vec::new(),
        Some(_) => read_lines(&mut archive, &mut budget, "sets.jsonl")?
            .into_iter()
            .map(|document| {
                let id = SetId(document.id.clone());
                let heads = heads_of(document, |body| set_from_body(&id, body).map(drop))?;
                Ok(IncomingSet { id, heads })
            })
            .collect::<anyhow::Result<_>>()?,
    };
    let pictures =
        picture_entries(&archive).iter().map(|name| AssetId::from(&name[PICTURES.len()..])).collect();
    Ok(Incoming { songs, churches, sets, pictures })
}

/// The names of the file's picture entries (`assets/<id>` with a well-formed id).
fn picture_entries<R: Read + Seek>(archive: &zip::ZipArchive<R>) -> Vec<String> {
    archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| name.strip_prefix(PICTURES).is_some_and(is_picture_id))
        .map(|name| name.into_owned())
        .collect()
}

/// A picture id as the store makes it: 64 hex digits and a known extension.
fn is_picture_id(id: &str) -> bool {
    id.split_once('.').is_some_and(|(hash, extension)| {
        hash.len() == 64
            && hash.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            && matches!(extension, "png" | "jpg" | "webp")
    })
}

/// How many of the file's pictures this device does not have yet (what storing them would add).
pub fn missing_pictures(incoming: &Incoming, data_dir: &Path) -> u32 {
    let missing = incoming.pictures.iter().filter(|id| !crate::assets::exists(data_dir, id)).count();
    u32::try_from(missing).unwrap_or(u32::MAX)
}

/// Stores the file's pictures this device does not have; returns how many. Each is read through
/// the store's size cap and must hash to the id it is filed under, so a file cannot plant a
/// picture under another one's id.
pub fn store_pictures(reader: impl Read + Seek, data_dir: &Path) -> anyhow::Result<u32> {
    let mut archive = zip::ZipArchive::new(reader).context("not a .sursum file")?;
    let mut added = 0;
    for name in picture_entries(&archive) {
        let id = AssetId::from(&name[PICTURES.len()..]);
        if crate::assets::exists(data_dir, &id) {
            continue;
        }
        let entry = archive.by_name(&name)?;
        let bytes = crate::limits::read_capped(entry, crate::assets::MAX_ASSET_BYTES as u64, &name)?;
        let stored = crate::assets::store(data_dir, &bytes).with_context(|| format!("picture {name}"))?;
        anyhow::ensure!(stored == id, "picture {name} does not match its id");
        added += 1;
    }
    Ok(added)
}

/// The documents of one `.jsonl` entry.
fn read_lines<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    budget: &mut crate::limits::Budget,
    name: &str,
) -> anyhow::Result<Vec<FileDocument>> {
    let lines = budget.read(archive.by_name(name).with_context(|| format!("no {name}"))?, name)?;
    let lines = std::str::from_utf8(&lines).with_context(|| format!("{name} is not UTF-8"))?;
    lines.lines().filter(|line| !line.trim().is_empty()).map(|line| Ok(serde_json::from_str(line)?)).collect()
}

/// A document's heads, checked: a well-formed id, a sane number of heads, well-formed ancestor
/// ids, and bodies that `readable` accepts.
fn heads_of(document: FileDocument, readable: impl Fn(&[u8]) -> anyhow::Result<()>) -> anyhow::Result<Heads> {
    anyhow::ensure!(is_document_id(&document.id), "an id in the file is malformed");
    anyhow::ensure!(
        (1..=MAX_HEADS).contains(&document.heads.len()),
        "{} has {} versions in the file",
        document.id,
        document.heads.len()
    );
    let versions = document
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
                    readable(text.as_bytes()).with_context(|| format!("{} in the file", document.id))?;
                    Body::Live(text.into_bytes())
                }
                None => Body::Deleted,
            };
            Ok(Version::new(body, ancestors))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Heads::of(versions))
}

/// Ids are made by this program (`song-<uuid>`, `opensong-<hex>`, `demo-…`, `church-<hex>`):
/// letters, digits, `-`, `_` and `.`, at most 128 characters.
fn is_document_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

/// Merges a file's songs and churches. With `dry_run` nothing is written, and the report says
/// exactly what a real merge would do (churches follow the same rules and are not reported).
///
/// Everything happens in one transaction (all or nothing, and one disk sync on the SD card).
///
/// `ours` tells the sets this device shows (its church's) from those it only keeps and passes on;
/// the report counts the latter in one number.
pub fn merge(
    database: &mut Database,
    incoming: &Incoming,
    dry_run: bool,
    ours: &dyn Fn(&ServiceSet) -> bool,
) -> anyhow::Result<MergeReport> {
    if dry_run {
        return merge_in(database, incoming, true, ours);
    }
    database.in_transaction(|database| merge_in(database, incoming, false, ours))
}

fn merge_in(
    database: &mut Database,
    incoming: &Incoming,
    dry_run: bool,
    ours: &dyn Fn(&ServiceSet) -> bool,
) -> anyhow::Result<MergeReport> {
    if !dry_run {
        for church in &incoming.churches {
            let before = database.church_heads(&church.id)?;
            let after = before.merge(&church.heads);
            if after != before {
                database.store_church_heads(&church.id, &before, &after)?;
            }
        }
    }
    let mut report = MergeReport::default();
    for song in &incoming.songs {
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
    for set in &incoming.sets {
        let before = database.set_heads(&set.id)?;
        let after = before.merge(&set.heads);
        // The version shown, or (for a deleted set) any version, to know whose set it is.
        let shown = after
            .current()
            .and_then(Version::live_body)
            .or_else(|| after.versions().iter().chain(before.versions()).find_map(Version::live_body))
            .map(|body| set_from_body(&set.id, body))
            .transpose()?;
        let is_ours = shown.as_ref().is_some_and(ours);
        match outcome(&before, &set.heads, &after) {
            Outcome::Unchanged | Outcome::KnownHere | Outcome::UnknownDeletion => {}
            _ if !is_ours => report.other_church_sets += 1,
            Outcome::Added => report.sets_added += 1,
            Outcome::Updated | Outcome::Deleted => report.sets_updated += 1,
            Outcome::ChangedInBoth => {
                report.sets_to_review.push(shown.map(|set| set.name).unwrap_or_default());
            }
        }
        if !dry_run && after != before {
            database.store_set_heads(&set.id, &before, &after)?;
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

    fn file_of(database: &Database) -> Incoming {
        read_bundle(std::io::Cursor::new(export(database, std::path::Path::new("")).unwrap())).unwrap()
    }

    fn sync(from: &Database, to: &mut Database) -> MergeReport {
        merge(to, &file_of(from), false, &|_| true).unwrap()
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
        let dry = merge(&mut tulce, &file, true, &|_| true).unwrap();
        assert_eq!((dry.updated, dry.unchanged), (1, 1));
        assert_eq!(text_of(&tulce, "a"), "pierwsza wersja", "a dry run writes nothing");
        assert_eq!(merge(&mut tulce, &file, false, &|_| true).unwrap(), dry, "the dry run said what happens");
        assert_eq!(text_of(&tulce, "a"), "poprawiona wersja");

        // An old file changes nothing; loading the same file again changes nothing.
        let old = merge(&mut tulce, &first_file, false, &|_| true).unwrap();
        assert_eq!((old.kept_local, old.updated), (1, 0), "{old:?}");
        assert_eq!(merge(&mut tulce, &file, false, &|_| true).unwrap().unchanged, 2);
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
        let report = merge(&mut tulce, &deletions, false, &|_| true).unwrap();
        assert_eq!((report.deleted, report.kept_local), (1, 1), "{report:?}");
        assert_eq!(live_ids(&tulce), ["d"], "the corrected song stays");
        assert_eq!(
            merge(&mut tulce, &deletions, false, &|_| true).unwrap().deleted,
            0,
            "nothing more to delete"
        );

        // Back the other way: the corrected "d" returns to Gowarzewo, "c" stays deleted.
        let report = sync(&tulce, &mut gowarzewo);
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(live_ids(&gowarzewo), ["d"]);
        assert_eq!(text_of(&gowarzewo, "d"), "poprawiona w Tulcach");
    }

    #[test]
    fn churches_travel_in_the_file_and_a_rename_follows() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        let tulce_id = tulce.name_church("Tulce").unwrap().unwrap();
        gowarzewo.name_church("Gowarzewo").unwrap();
        sync(&tulce, &mut gowarzewo);
        sync(&gowarzewo, &mut tulce);
        let names = |database: &Database| -> Vec<String> {
            database.churches().unwrap().into_iter().map(|church| church.name).collect()
        };
        assert_eq!(names(&tulce), ["Gowarzewo", "Tulce"]);
        assert_eq!(names(&gowarzewo), ["Gowarzewo", "Tulce"]);
        assert_eq!(
            gowarzewo.this_church().unwrap().map(|id| id == tulce_id),
            Some(false),
            "each keeps its own"
        );

        tulce.name_church("Parafia Tulce").unwrap();
        sync(&tulce, &mut gowarzewo);
        assert_eq!(names(&gowarzewo), ["Gowarzewo", "Parafia Tulce"]);
    }

    fn service_set(id: &str, name: &str, church: &ChurchId) -> ServiceSet {
        ServiceSet {
            id: SetId::from(id),
            name: name.to_owned(),
            items: vec![],
            date: Some("2026-11-22".to_owned()),
            church: Some(church.clone()),
        }
    }

    fn set_names(database: &Database) -> Vec<String> {
        database.load_sets().unwrap().into_iter().map(|set| set.name).collect()
    }

    #[test]
    fn sets_travel_and_only_this_churchs_sets_are_reported_as_its_own() {
        let (mut tulce, mut gowarzewo) = (Database::in_memory().unwrap(), Database::in_memory().unwrap());
        let tulce_id = tulce.name_church("Tulce").unwrap().unwrap();
        let gowarzewo_id = gowarzewo.name_church("Gowarzewo").unwrap().unwrap();
        tulce.save_set(&service_set("niedziela", "Niedziela", &tulce_id)).unwrap();
        let theirs = |set: &ServiceSet| set.church.as_ref() == Some(&gowarzewo_id);

        let report = merge(&mut gowarzewo, &file_of(&tulce), false, &theirs).unwrap();
        assert_eq!((report.sets_added, report.other_church_sets), (0, 1), "{report:?}");
        assert_eq!(set_names(&gowarzewo), ["Niedziela"], "kept, so it can travel on");

        // Back in Tulce (the organist's own set), a change made elsewhere is reported as such.
        let mut renamed = service_set("niedziela", "Niedziela, suma", &tulce_id);
        renamed.date = Some("2026-11-22T11:00".to_owned());
        gowarzewo.save_set(&renamed).unwrap();
        let ours = |set: &ServiceSet| set.church.as_ref() == Some(&tulce_id);
        let report = merge(&mut tulce, &file_of(&gowarzewo), false, &ours).unwrap();
        assert_eq!(report.sets_updated, 1, "{report:?}");
        assert_eq!(set_names(&tulce), ["Niedziela, suma"]);

        // Changed in both places: one set, the one with more changes shown, waiting for review.
        tulce.save_set(&service_set("niedziela", "Tulce 1", &tulce_id)).unwrap();
        tulce.save_set(&service_set("niedziela", "Tulce 2", &tulce_id)).unwrap();
        gowarzewo.save_set(&service_set("niedziela", "Gowarzewo", &tulce_id)).unwrap();
        let report = merge(&mut tulce, &file_of(&gowarzewo), false, &ours).unwrap();
        assert_eq!(report.sets_to_review, ["Tulce 2"], "{report:?}");
        sync(&tulce, &mut gowarzewo);
        assert_eq!(set_names(&tulce), ["Tulce 2"]);
        assert_eq!(set_names(&gowarzewo), ["Tulce 2"]);

        // A deletion travels too.
        tulce.delete_set("niedziela").unwrap();
        let report = merge(&mut gowarzewo, &file_of(&tulce), false, &theirs).unwrap();
        assert_eq!(report.other_church_sets, 1, "{report:?}");
        assert!(set_names(&gowarzewo).is_empty());
    }

    /// Files of Tulce after each of three edits of song "s" (v1, v2, v3).
    fn three_files() -> Vec<Incoming> {
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
        merge(&mut gowarzewo, &files[1], false, &|_| true).unwrap();
        gowarzewo.delete_song("s").unwrap();
        let report = merge(&mut gowarzewo, &files[0], false, &|_| true).unwrap();
        assert_eq!(report.updated, 0, "{report:?}");
        assert!(live_ids(&gowarzewo).is_empty(), "v1 is older than the deleted v2");
        // A version made after v2 without knowing of the deletion does come back.
        let report = merge(&mut gowarzewo, &files[2], false, &|_| true).unwrap();
        assert_eq!(report.updated, 1, "{report:?}");
        assert_eq!(text_of(&gowarzewo, "s"), "v3");
    }

    #[test]
    fn files_merged_out_of_order_give_the_newest_version() {
        let files = three_files();
        let mut gowarzewo = Database::in_memory().unwrap();
        for file in [&files[0], &files[2], &files[1]] {
            let report = merge(&mut gowarzewo, file, false, &|_| true).unwrap();
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
        assert!(read_bundle(std::io::Cursor::new(b"nope")).is_err());
        let old = zip_of(&[("manifest.json", br#"{"format":1,"songs":0}"#, 1), ("songs.jsonl", b"", 1)]);
        let error = read_bundle(std::io::Cursor::new(&old)).unwrap_err().to_string();
        assert!(error.contains("format 1"), "{error}");
    }

    #[test]
    fn nothing_in_the_file_is_trusted() {
        let body = String::from_utf8(crate::db::song_body(&song("a", "z pliku")).unwrap()).unwrap();
        let good = serde_json::json!({ "id": "a", "heads": [{ "body": body, "ancestors": [] }] });
        let read = read_bundle(std::io::Cursor::new(file_with_line(&good))).unwrap();
        assert_eq!(
            read.songs[0].heads.current().unwrap().id(),
            Version::first(body.clone().into_bytes()).id()
        );

        let bad_ancestor =
            serde_json::json!({ "id": "a", "heads": [{ "body": body, "ancestors": ["nie-hash"] }] });
        let not_a_song = serde_json::json!({ "id": "a", "heads": [{ "body": "[1, 2]", "ancestors": [] }] });
        let bad_id = serde_json::json!({ "id": "a b/../c", "heads": [{ "body": body, "ancestors": [] }] });
        let no_heads = serde_json::json!({ "id": "a", "heads": [] });
        for line in [bad_ancestor, not_a_song, bad_id, no_heads] {
            assert!(read_bundle(std::io::Cursor::new(file_with_line(&line))).is_err(), "{line}");
        }
    }

    #[test]
    fn a_zip_bomb_is_refused_without_unpacking_it() {
        let manifest = br#"{"format":2,"songs":1}"#;
        let zeros = vec![b' '; 1024 * 1024];
        let bytes = zip_of(&[("manifest.json", manifest, 1), ("songs.jsonl", &zeros, 70)]);
        assert!(bytes.len() < 1024 * 1024, "small on the wire");
        let error = read_bundle(std::io::Cursor::new(&bytes)).unwrap_err().to_string();
        assert!(error.contains("larger than"), "{error}");
    }
}
