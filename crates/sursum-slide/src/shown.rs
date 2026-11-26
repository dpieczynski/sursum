// SPDX-License-Identifier: GPL-3.0-or-later
//! Which slide to draw for a live position. During the gap after an item's last slide the
//! screen draws an empty slide on the deck's background, so the congregation sees neither the
//! old words nor the next song's.

use sursum_protocol::{LiveSlide, Slide, SlideBody, SlideKey, SlidePart};

/// The slide to draw: the one at `slide_index`, or an empty slide during the gap. The gap's
/// key differs from every real key, so moving into or out of the gap is a normal transition.
pub fn shown_slide(live: &LiveSlide) -> Option<Slide> {
    let slide = live.deck.slides.get(live.slide_index as usize)?;
    if !live.gap {
        return Some(slide.clone());
    }
    Some(Slide {
        key: SlideKey(format!("{}~gap", slide.key.as_str())),
        part: SlidePart::Pause,
        body: SlideBody::Text { title: None, lines: Vec::new() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_protocol::{Deck, DeckId, ItemId};

    fn live(gap: bool) -> LiveSlide {
        let slide = Slide {
            key: SlideKey("v1#0@0".into()),
            part: SlidePart::Page { number: 1 },
            body: SlideBody::Lyrics { lines: vec!["a".into()] },
        };
        let deck = Deck {
            deck_id: DeckId("d".into()),
            fit_group: "d".into(),
            item_id: ItemId::from("i"),
            title: "t".into(),
            slides: vec![slide],
        };
        LiveSlide { deck, slide_index: 0, gap }
    }

    #[test]
    fn the_gap_is_an_empty_slide_with_its_own_key() {
        assert_eq!(shown_slide(&live(false)).unwrap().key.as_str(), "v1#0@0");
        let gap = shown_slide(&live(true)).unwrap();
        assert_eq!(gap.key.as_str(), "v1#0@0~gap");
        assert_eq!(gap.body, SlideBody::Text { title: None, lines: Vec::new() });
    }
}
