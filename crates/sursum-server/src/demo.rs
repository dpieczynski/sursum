// SPDX-License-Identifier: GPL-3.0-or-later
//! Demo content used until the song library and sets exist (milestones M2 and M3): public-domain
//! and synthetic songs from `fixtures/demo`, arranged into one set.

use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song};
use sursum_protocol::{ItemId, SetId};

const DEMO_SONGS: &[&str] = &[
    include_str!("../../../fixtures/demo/kiedy-ranne-wstaja-zorze.json"),
    include_str!("../../../fixtures/demo/synthetic-refrain.json"),
];

/// The demo songs. Panics only if the bundled fixtures are malformed (caught by tests).
pub fn songs() -> Vec<Song> {
    DEMO_SONGS
        .iter()
        .map(|json| serde_json::from_str(json).expect("bundled demo song is valid JSON"))
        .collect()
}

/// A demo set: both songs and an announcement slide.
pub fn set(songs: &[Song]) -> ServiceSet {
    let mut items: Vec<SetItem> = songs
        .iter()
        .enumerate()
        .map(|(index, song)| SetItem {
            id: ItemId(format!("demo-item-{}", index + 1)),
            kind: ItemKind::Song { song_id: song.id.clone(), order_override: None },
        })
        .collect();
    items.push(SetItem {
        id: ItemId(format!("demo-item-{}", items.len() + 1)),
        kind: ItemKind::Text {
            title: "Ogłoszenia".to_owned(),
            slides: vec!["Zapraszamy na adorację\nw czwartek o 19:00".to_owned()],
        },
    });
    ServiceSet { id: SetId::from("demo-set"), name: "Msza (przykład)".to_owned(), items }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_songs_parse_and_form_a_set() {
        let songs = songs();
        assert_eq!(songs.len(), 2);
        assert_eq!(set(&songs).items.len(), 3);
    }
}
