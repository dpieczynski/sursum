// SPDX-License-Identifier: GPL-3.0-or-later
//! Importing OpenSong songs and sets, from a folder (`sursum-server import-opensong`) or from a
//! ZIP file uploaded in the browser. Importing the same songs again only updates the changed ones;
//! importing the same set file again replaces that set.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

use anyhow::Context as _;
use sha2::{Digest as _, Sha256};
use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song};
use sursum_formats::opensong::{
    ImportedSet, LibraryImport, SetEntry, add_file, add_set_file, is_skipped, presentation_for, read_library,
    read_sets,
};
use sursum_protocol::{ImportReport, ItemId, SetId, SongId};

use crate::db::{Database, Upsert};

/// At most this many problem lines are reported back.
const MAX_PROBLEMS: usize = 500;

/// Imports all songs from an OpenSong `Songs` folder, and the sets from `Sets` next to it.
pub fn import_opensong(
    database: &mut Database,
    songs_dir: &Path,
    songbook: &str,
) -> anyhow::Result<ImportReport> {
    let mut library =
        read_library(songs_dir, songbook).with_context(|| format!("reading {}", songs_dir.display()))?;
    let sets_dir = songs_dir.join("../Sets");
    if sets_dir.is_dir() {
        read_sets(&mut library, &sets_dir).with_context(|| format!("reading {}", sets_dir.display()))?;
    }
    store(database, &library).map(|(report, _)| report)
}

/// Reads the songs in a ZIP of an OpenSong folder. Paths are taken relative to the last `Songs`
/// folder in the archive (so zipping `OpenSong/`, `Songs/` or the songs themselves all work).
pub fn read_zip(bytes: &[u8], songbook: &str) -> anyhow::Result<LibraryImport> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a ZIP file")?;
    let mut import = LibraryImport::default();
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.replace('\\', "/");
        let kind = match (song_path(&name), set_file_name(&name)) {
            (Some(relative), _) => FileKind::Song(relative),
            (None, Some(file_name)) => FileKind::Set(file_name),
            (None, None) => continue,
        };
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        files.push((kind, content));
    }
    files.sort();
    for (kind, content) in files {
        match kind {
            FileKind::Song(relative) => add_file(&mut import, relative, &content, songbook),
            FileKind::Set(file_name) => add_set_file(&mut import, &file_name, &content),
        }
    }
    Ok(import)
}

/// What a file in an uploaded archive is.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FileKind {
    Song(String),
    Set(String),
}

/// The file name of a set inside the archive (a file directly in a `Sets` folder), or `None`.
fn set_file_name(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split('/').filter(|part| !part.is_empty()).collect();
    let [.., folder, file] = parts.as_slice() else { return None };
    let hidden = parts.iter().any(|part| part.starts_with('.') || *part == "__MACOSX");
    (folder.eq_ignore_ascii_case("sets") && !hidden && !is_skipped(file)).then(|| (*file).to_owned())
}

/// The path of a song inside the archive, or `None` for files that are not songs.
fn song_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split('/').filter(|part| !part.is_empty()).collect();
    let hidden = parts.iter().any(|part| part.starts_with('.') || *part == "__MACOSX");
    let file = parts.last()?;
    if hidden || is_skipped(file) {
        return None;
    }
    // Outside a `Songs` folder (e.g. `Sets/`, `Settings/`) nothing is a song, unless the archive
    // has no `Songs` folder at all.
    match parts.iter().rposition(|part| part.eq_ignore_ascii_case("songs")) {
        Some(index) => Some(parts[index + 1..].join("/")).filter(|path| !path.is_empty()),
        None if parts
            .iter()
            .any(|part| ["sets", "settings", "backgrounds"].contains(&part.to_lowercase().as_str())) =>
        {
            None
        }
        None => Some(parts.join("/")),
    }
}

/// Stores imported songs, then the sets (resolved against the whole library), and reports what
/// happened. Returns the stored sets so a running server can show them without reloading.
pub fn store(
    database: &mut Database,
    library: &LibraryImport,
) -> anyhow::Result<(ImportReport, Vec<ServiceSet>)> {
    let mut report = ImportReport { failed: count(library.failures.len()), ..ImportReport::default() };
    for (path, imported) in &library.songs {
        for warning in &imported.warnings {
            push_problem(&mut report, format!("{path}: {warning}"));
        }
        match database.upsert_song(&imported.song, &format!("opensong:{path}"))? {
            Upsert::Inserted => report.inserted += 1,
            Upsert::Updated { .. } => report.updated += 1,
            Upsert::Unchanged => report.unchanged += 1,
        }
    }
    for (path, reason) in &library.failures {
        push_problem(&mut report, format!("{path}: {reason}"));
    }
    let mut sets = Vec::new();
    if !library.sets.is_empty() {
        let songs: HashMap<SongId, Song> =
            database.load_songs()?.into_iter().map(|song| (song.id.clone(), song)).collect();
        let finder = SongFinder::new(&songs);
        for imported in &library.sets {
            let (set, problems) = resolve_set(imported, &finder);
            for problem in problems {
                push_problem(&mut report, format!("Sets/{}: {problem}", imported.file_name));
            }
            database.save_set(&set)?;
            report.sets += 1;
            sets.push(set);
        }
    }
    Ok((report, sets))
}

/// Finds the library song a set entry names.
struct SongFinder<'a> {
    songs: &'a HashMap<SongId, Song>,
    /// OpenSong path inside `Songs/` → song (only imported songs have one).
    by_path: HashMap<&'a str, &'a SongId>,
}

impl<'a> SongFinder<'a> {
    fn new(songs: &'a HashMap<SongId, Song>) -> Self {
        let by_path = songs
            .iter()
            .filter_map(|(id, song)| song.extra.get("opensong_path").map(|path| (path.as_str(), id)))
            .collect();
        SongFinder { songs, by_path }
    }

    /// By the recorded path, else by file name (OpenSong file name = title), else by title (songs
    /// typed in sursum's editor). With several candidates the one with the shortest path wins, so a
    /// song in the root folder is preferred; that choice is reported.
    fn find(&self, entry: &SetEntry) -> Result<(&'a Song, Option<String>), String> {
        let SetEntry::Song { name, .. } = entry else { return Err("not a song".into()) };
        if let Some(id) = entry.song_path().and_then(|path| self.by_path.get(path.as_str()).copied()) {
            return Ok((&self.songs[id], None));
        }
        let mut candidates: Vec<(&str, &SongId)> = self
            .by_path
            .iter()
            .filter(|(path, _)| path.rsplit('/').next() == Some(name.as_str()))
            .map(|(path, id)| (*path, *id))
            .collect();
        if candidates.is_empty() {
            candidates = self
                .songs
                .values()
                .filter(|song| song.title().trim().eq_ignore_ascii_case(name))
                .map(|song| (song.title(), &song.id))
                .collect();
        }
        candidates.sort_by_key(|(path, id)| (path.len(), *path, id.as_str()));
        match candidates.as_slice() {
            [] => Err(format!("song {name:?} is not in the library; skipped")),
            [(_, id)] => Ok((&self.songs[*id], None)),
            [(path, id), ..] => Ok((
                &self.songs[*id],
                Some(format!("{} songs are named {name:?}; used {path:?}", candidates.len())),
            )),
        }
    }
}

/// Turns an imported set into a sursum set. Ids derive from the file name, so importing the same
/// file again replaces the set and keeps item ids (the program can still find its live item).
fn resolve_set(imported: &ImportedSet, finder: &SongFinder<'_>) -> (ServiceSet, Vec<String>) {
    let digest = Sha256::digest(imported.file_name.as_bytes());
    let hex: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    let set_id = SetId(format!("opensong-set-{hex}"));
    let mut problems = imported.skipped.clone();
    let mut items = Vec::new();
    for (index, entry) in imported.entries.iter().enumerate() {
        let kind = match entry {
            SetEntry::Text { title, pages } => ItemKind::Text { title: title.clone(), slides: pages.clone() },
            SetEntry::Song { presentation, .. } => match finder.find(entry) {
                Ok((song, note)) => {
                    problems.extend(note);
                    let order_override = presentation.as_deref().map(|text| {
                        let (order, warnings) = presentation_for(song, text);
                        problems
                            .extend(warnings.iter().map(|warning| format!("{}: {warning}", song.title())));
                        order
                    });
                    ItemKind::Song { song_id: song.id.clone(), order_override }
                }
                Err(problem) => {
                    problems.push(problem);
                    continue;
                }
            },
        };
        items.push(SetItem { id: ItemId(format!("{}-{index}", set_id.as_str())), kind });
    }
    (ServiceSet { id: set_id, name: imported.name.clone(), items }, problems)
}

fn push_problem(report: &mut ImportReport, line: String) {
    if report.problems.len() < MAX_PROBLEMS {
        report.problems.push(line);
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/opensong/Songs")
    }

    #[test]
    fn importing_the_fixture_library_twice_is_idempotent() {
        let mut database = Database::in_memory().unwrap();
        let first = import_opensong(&mut database, &fixtures(), "Przykładowy").unwrap();
        assert_eq!((first.inserted, first.failed), (3, 1));
        assert!(first.problems.iter().any(|line| line.starts_with("Synthetic Broken")));
        let second = import_opensong(&mut database, &fixtures(), "Przykładowy").unwrap();
        assert_eq!((second.inserted, second.updated, second.unchanged), (0, 0, 3));
        assert_eq!(database.load_songs().unwrap().len(), 3);
    }

    #[test]
    fn zip_of_an_opensong_folder_imports_like_the_folder() {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for name in ["Synthetic Basic", "Kolędy/Synthetic Numbered", "cover.jpg"] {
            writer.start_file(format!("OpenSong/Songs/{name}"), options).unwrap();
            writer.write_all(&std::fs::read(fixtures().join(name)).unwrap()).unwrap();
        }
        writer.start_file("OpenSong/Sets/Niedziela", options).unwrap();
        writer.write_all(b"<set/>").unwrap();
        writer.finish().unwrap();
        let library = read_zip(buffer.get_ref(), "Przykładowy").unwrap();
        let paths: Vec<&str> = library.songs.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, ["Kolędy/Synthetic Numbered", "Synthetic Basic"]);
        assert!(library.failures.is_empty());
    }

    #[test]
    fn song_paths_inside_archives() {
        assert_eq!(song_path("OpenSong/Songs/A/B").as_deref(), Some("A/B"));
        assert_eq!(song_path("Songs/X").as_deref(), Some("X"));
        assert_eq!(song_path("X").as_deref(), Some("X"));
        assert_eq!(song_path("OpenSong/Sets/S"), None);
        assert_eq!(song_path("__MACOSX/Songs/._X"), None);
        assert_eq!(song_path("Songs/cover.JPG"), None);
        assert_eq!(set_file_name("OpenSong/Sets/Niedziela").as_deref(), Some("Niedziela"));
        assert_eq!(set_file_name("Sets/Niedziela").as_deref(), Some("Niedziela"));
        assert_eq!(set_file_name("Songs/Sets"), None);
        assert_eq!(set_file_name("Niedziela"), None);
    }

    #[test]
    fn sets_are_resolved_against_the_library_and_reimport_replaces_them() {
        let mut database = Database::in_memory().unwrap();
        let first = import_opensong(&mut database, &fixtures(), "Przykładowy").unwrap();
        assert_eq!(first.sets, 1);
        assert!(
            first.problems.iter().any(|line| line.contains("Pieśń której nie ma")),
            "{:?}",
            first.problems
        );
        assert!(first.problems.iter().any(|line| line.contains("image")), "{:?}", first.problems);
        let sets = database.load_sets().unwrap();
        let set = &sets[0];
        assert_eq!(set.name, "Syntetyczna niedziela");
        let kinds: Vec<&str> = set
            .items
            .iter()
            .map(|item| match &item.kind {
                ItemKind::Song { order_override: Some(_), .. } => "song+order",
                ItemKind::Song { .. } => "song",
                ItemKind::Text { .. } => "text",
            })
            .collect();
        assert_eq!(kinds, ["song+order", "text", "song", "text"]);
        let ItemKind::Song { song_id, .. } = &set.items[2].kind else { unreachable!() };
        let songs = database.load_songs().unwrap();
        let numbered = songs.iter().find(|song| &song.id == song_id).unwrap();
        assert_eq!(numbered.extra["opensong_path"], "Kolędy/Synthetic Numbered", "found by file name");

        import_opensong(&mut database, &fixtures(), "Przykładowy").unwrap();
        let again = database.load_sets().unwrap();
        assert_eq!(again.len(), 1, "re-import replaces the set");
        assert_eq!(again[0].items, set.items, "with the same item ids");
    }

    #[test]
    fn garbage_is_not_a_zip() {
        assert!(read_zip(b"not a zip", "S").is_err());
    }
}
