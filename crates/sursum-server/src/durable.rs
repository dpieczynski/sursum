// SPDX-License-Identifier: GPL-3.0-or-later
//! Making file system changes survive a power cut.

use std::path::Path;

/// Makes the renames and new files in `dir` durable. On Windows a directory cannot be opened as a
/// file (and NTFS keeps its own metadata journal), so there it does nothing.
pub fn sync_dir(dir: &Path) -> std::io::Result<()> {
    if cfg!(unix) { std::fs::File::open(dir)?.sync_all() } else { Ok(()) }
}

/// Makes the contents of the file at `path` durable, for files written by someone else (SQLite's
/// `VACUUM INTO`, `std::fs::copy`). Opened for writing: Windows refuses to flush a read-only
/// handle ("Access is denied").
pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).open(path)?.sync_all()
}
