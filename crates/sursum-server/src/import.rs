// SPDX-License-Identifier: GPL-3.0-or-later
//! `sursum-server import-opensong`: imports an OpenSong `Songs/` folder into the database and
//! prints a report. Importing the same folder again only updates changed songs.

use std::path::Path;

use anyhow::Context as _;
use sursum_formats::opensong::read_library;

use crate::db::{Database, Upsert};

/// Counts of what an import did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub failed: usize,
    pub warnings: usize,
}

/// Imports all songs from `songs_dir`; `songbook` names the songbook of OpenSong `hymn_number`s.
/// Problems are written to `report` (one line each).
pub fn import_opensong(
    database: &mut Database,
    songs_dir: &Path,
    songbook: &str,
    report: &mut dyn std::io::Write,
) -> anyhow::Result<ImportSummary> {
    let library =
        read_library(songs_dir, songbook).with_context(|| format!("reading {}", songs_dir.display()))?;
    let mut summary = ImportSummary { failed: library.failures.len(), ..ImportSummary::default() };
    for (path, imported) in &library.songs {
        for warning in &imported.warnings {
            writeln!(report, "warning: {path}: {warning}")?;
            summary.warnings += 1;
        }
        match database.upsert_song(&imported.song, &format!("opensong:{path}"))? {
            Upsert::Inserted => summary.inserted += 1,
            Upsert::Updated { .. } => summary.updated += 1,
            Upsert::Unchanged => summary.unchanged += 1,
        }
    }
    for (path, reason) in &library.failures {
        writeln!(report, "failed: {path}: {reason}")?;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_the_fixture_library_twice_is_idempotent() {
        let songs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/opensong/Songs");
        let mut database = Database::in_memory().unwrap();
        let mut report = Vec::new();
        let first = import_opensong(&mut database, &songs_dir, "Przykładowy", &mut report).unwrap();
        assert_eq!((first.inserted, first.failed), (3, 1));
        let second = import_opensong(&mut database, &songs_dir, "Przykładowy", &mut Vec::new()).unwrap();
        assert_eq!((second.inserted, second.updated, second.unchanged), (0, 0, 3));
        assert_eq!(database.load_songs().unwrap().len(), 3);
        assert!(String::from_utf8(report).unwrap().contains("failed: Synthetic Broken"));
    }
}
