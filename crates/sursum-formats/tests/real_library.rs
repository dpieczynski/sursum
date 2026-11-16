// SPDX-License-Identifier: GPL-3.0-or-later
//! Imports a real OpenSong library from `SURSUM_REAL_LIBRARY` (a `Songs/` folder kept outside the
//! repository, e.g. in `/local/`). Prints statistics only, never song text:
//! `SURSUM_REAL_LIBRARY=local/OpenSong/Songs cargo test -p sursum-formats --test real_library -- --ignored --nocapture`

use std::collections::BTreeMap;

use sursum_core::slides::{SlideRules, compile_song};
use sursum_formats::opensong::read_library;
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
}
