// SPDX-License-Identifier: GPL-3.0-or-later
//! Imports a real OpenSong library from `SURSUM_REAL_LIBRARY` (a `Songs/` folder kept outside the
//! repository, e.g. in `/local/`). Prints statistics only, never song text:
//! `SURSUM_REAL_LIBRARY=local/OpenSong/Songs cargo test -p sursum-formats --test real_library -- --ignored --nocapture`

use std::collections::BTreeMap;

use sursum_core::slides::{SlideRules, compile_song};
use sursum_formats::opensong::{SetEntry, read_library, read_sets};
use sursum_protocol::ItemId;

#[test]
#[ignore = "needs SURSUM_REAL_LIBRARY"]
fn real_library_imports_without_failures() {
    let Some(dir) = std::env::var_os("SURSUM_REAL_LIBRARY") else {
        eprintln!("SURSUM_REAL_LIBRARY not set; skipping");
        return;
    };
    let import = read_library(std::path::Path::new(&dir), "OpenSong").expect("library folder readable");
    let mut warning_kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut slides = 0;
    for (_, song) in &import.songs {
        for warning in &song.warnings {
            let kind = format!("{:?}", warning.kind);
            let kind = kind.split('(').next().unwrap_or_default().to_owned();
            *warning_kinds.entry(kind).or_default() += 1;
        }
        slides += compile_song(&song.song, &ItemId::from("x"), None, SlideRules::default()).slides.len();
    }
    println!("songs: {}, failures: {}, slides: {slides}", import.songs.len(), import.failures.len());
    println!("warnings by kind: {warning_kinds:?}");
    for (path, reason) in import.failures.iter().take(10) {
        println!("failed: {} chars in path, reason: {reason}", path.chars().count());
    }
    assert!(import.failures.is_empty(), "{} files failed to import", import.failures.len());

    // Sets next to `Songs/`: how many song entries can be found, by path or by file name.
    let mut import = import;
    let sets_dir = std::path::Path::new(&dir).join("../Sets");
    if sets_dir.is_dir() {
        read_sets(&mut import, &sets_dir).expect("sets folder readable");
        let paths: Vec<&str> = import.songs.iter().map(|(path, _)| path.as_str()).collect();
        let (mut by_path, mut by_name, mut missing, mut texts, mut skipped) = (0, 0, 0, 0, 0);
        for set in &import.sets {
            skipped += set.skipped.len();
            for entry in &set.entries {
                match entry {
                    SetEntry::Text { .. } => texts += 1,
                    SetEntry::Song { name, .. } => {
                        if entry.song_path().is_some_and(|path| paths.contains(&path.as_str())) {
                            by_path += 1;
                        } else if paths.iter().any(|path| path.rsplit('/').next() == Some(name.as_str())) {
                            by_name += 1;
                        } else {
                            missing += 1;
                        }
                    }
                }
            }
        }
        println!(
            "sets: {}, songs by path: {by_path}, by name: {by_name}, missing: {missing}, texts: {texts}, skipped groups: {skipped}, failures: {}",
            import.sets.len(),
            import.failures.len()
        );
        assert!(import.failures.is_empty(), "set files failed to import");
    }
}
