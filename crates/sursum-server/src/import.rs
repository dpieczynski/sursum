// SPDX-License-Identifier: GPL-3.0-or-later
//! Importing OpenSong songs, from a folder (`sursum-server import-opensong`) or from a ZIP file
//! uploaded in the browser. Importing the same songs again only updates the changed ones.

use std::io::Read as _;
use std::path::Path;

use anyhow::Context as _;
use sursum_formats::opensong::{LibraryImport, add_file, is_skipped, read_library};
use sursum_protocol::ImportReport;

use crate::db::{Database, Upsert};

/// At most this many problem lines are reported back.
const MAX_PROBLEMS: usize = 500;

/// Imports all songs from an OpenSong `Songs` folder.
pub fn import_opensong(
    database: &mut Database,
    songs_dir: &Path,
    songbook: &str,
) -> anyhow::Result<ImportReport> {
    let library =
        read_library(songs_dir, songbook).with_context(|| format!("reading {}", songs_dir.display()))?;
    store(database, &library)
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
        let Some(relative) = song_path(&name) else { continue };
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        files.push((relative, content));
    }
    files.sort();
    for (relative, content) in files {
        add_file(&mut import, relative, &content, songbook);
    }
    Ok(import)
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

/// Stores imported songs and reports what happened.
pub fn store(database: &mut Database, library: &LibraryImport) -> anyhow::Result<ImportReport> {
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
    Ok(report)
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
    }

    #[test]
    fn garbage_is_not_a_zip() {
        assert!(read_zip(b"not a zip", "S").is_err());
    }
}
