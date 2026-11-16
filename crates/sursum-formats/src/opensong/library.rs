// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading a whole OpenSong `Songs/` folder.

use std::path::{Path, PathBuf};

use super::song::{ImportedSong, parse_song};

/// Everything found in a `Songs/` folder.
#[derive(Debug, Default)]
pub struct LibraryImport {
    /// Imported songs with their path inside the folder (`/`-separated).
    pub songs: Vec<(String, ImportedSong)>,
    /// Files that could not be imported, with the reason.
    pub failures: Vec<(String, String)>,
}

/// File extensions that are never songs (images, databases) and are skipped silently.
const SKIPPED_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "gif", "bmp", "db", "ini", "pdf", "mp3", "wav", "zip"];

/// Parses one file into `import` (used for folders and for ZIP uploads).
pub fn add_file(import: &mut LibraryImport, relative: String, bytes: &[u8], songbook: &str) {
    match parse_song(bytes, &relative, songbook) {
        Ok(song) => import.songs.push((relative, song)),
        Err(error) => import.failures.push((relative, error.to_string())),
    }
}

/// True for files that are never songs (images, databases), judged by the extension.
pub fn is_skipped(path: &str) -> bool {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension.to_lowercase());
    extension.is_some_and(|extension| SKIPPED_EXTENSIONS.contains(&extension.as_str()))
}

/// Reads every song in `songs_dir` and its subfolders, in a stable (sorted) order.
pub fn read_library(songs_dir: &Path, songbook: &str) -> std::io::Result<LibraryImport> {
    let mut files = Vec::new();
    collect_files(songs_dir, &mut files)?;
    files.sort();
    let mut import = LibraryImport::default();
    for file in files {
        let relative = relative_path(songs_dir, &file);
        let result = std::fs::read(&file)
            .map_err(|error| error.to_string())
            .and_then(|bytes| parse_song(&bytes, &relative, songbook).map_err(|error| error.to_string()));
        match result {
            Ok(song) => import.songs.push((relative, song)),
            Err(reason) => import.failures.push((relative, reason)),
        }
    }
    Ok(import)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let hidden =
            path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, files)?;
            continue;
        }
        let skipped = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| SKIPPED_EXTENSIONS.contains(&extension.to_lowercase().as_str()));
        if !skipped {
            files.push(path);
        }
    }
    Ok(())
}

fn relative_path(root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file);
    relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}
