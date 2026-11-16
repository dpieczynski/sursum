// SPDX-License-Identifier: GPL-3.0-or-later
//! The DOM half of font fitting, in a real browser (`just test-wasm`): the pure binary search is
//! tested natively in `fit.rs`; here we check that measuring real text boxes gives sizes that
//! actually fit, shrink for longer lines and narrower areas, and do not depend on how the stage
//! is scaled (which is what makes the remote preview match the screen).

#![cfg(target_arch = "wasm32")]

use sursum_protocol::{Background, Deck, DeckId, ItemId, Rect, Slide, SlideBody, SlideKey, Theme};
use sursum_slide::fit::fit_deck;
use sursum_slide::slide_view::{lines_class, text_box_style};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const SLIDE_CSS: &str = include_str!("../style/slide.css");
const FULL: Rect = Rect::new(48, 48, 1824, 984);

fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}

/// A 1920×1080 stage with the slide stylesheet, scaled like a remote preview, and a measurer.
fn stage(scale: f64) -> web_sys::HtmlElement {
    let document = document();
    if document.get_element_by_id("slide-css").is_none() {
        let style = document.create_element("style").unwrap();
        style.set_id("slide-css");
        style.set_text_content(Some(SLIDE_CSS));
        document.head().unwrap().append_child(&style).unwrap();
    }
    let stage: web_sys::HtmlElement = document.create_element("div").unwrap().dyn_into().unwrap();
    stage.set_class_name("sursum-stage");
    stage.set_attribute("style", &format!("transform: scale({scale})")).unwrap();
    let measurer: web_sys::HtmlElement = document.create_element("div").unwrap().dyn_into().unwrap();
    measurer.set_class_name("sursum-layer sursum-measure");
    stage.append_child(&measurer).unwrap();
    document.body().unwrap().append_child(&stage).unwrap();
    measurer
}

/// A deck of one slide per entry; `group` keeps decks apart in the fit cache.
fn deck(group: &str, slides: &[&[&str]]) -> Deck {
    Deck {
        deck_id: DeckId(group.to_owned()),
        fit_group: group.to_owned(),
        item_id: ItemId::from("item"),
        title: group.to_owned(),
        slides: slides
            .iter()
            .enumerate()
            .map(|(index, lines)| Slide {
                key: SlideKey(format!("v{index}")),
                label: String::new(),
                body: SlideBody::Lyrics { lines: lines.iter().map(|line| (*line).to_owned()).collect() },
            })
            .collect(),
        background: Background::default(),
    }
}

/// Draws `lines` at `font_px` exactly like the screen and reports whether they overflow.
fn overflows(measurer: &web_sys::HtmlElement, lines: &[&str], content: Rect, font_px: u32) -> bool {
    let theme = Theme::default();
    let document = document();
    let text: web_sys::HtmlElement = document.create_element("div").unwrap().dyn_into().unwrap();
    text.set_class_name(&lines_class(&theme, false));
    text.set_attribute(
        "style",
        &text_box_style(&theme, Rect::new(0, 0, content.width, content.height), font_px),
    )
    .unwrap();
    for line in lines {
        let row = document.create_element("div").unwrap();
        row.set_class_name("sursum-line");
        row.set_text_content(Some(line));
        text.append_child(&row).unwrap();
    }
    measurer.append_child(&text).unwrap();
    let overflow = text.scroll_width() > text.client_width() || text.scroll_height() > text.client_height();
    text.remove();
    overflow
}

const SHORT: &[&str] = &["Krótka linia", "druga"];
const LONG: &[&str] = &["Ta linia jest wyraźnie dłuższa od pozostałych, żeby wymusić mniejszą czcionkę"];

#[wasm_bindgen_test]
async fn the_chosen_size_fits_and_one_size_more_does_not() {
    let measurer = stage(1.0);
    let fit = fit_deck(&deck("fits", &[SHORT, LONG]), &Theme::default(), FULL, &measurer).await;
    assert!(!fit.wrap);
    assert!(!overflows(&measurer, LONG, FULL, fit.font_px), "{fit:?} overflows");
    assert!(overflows(&measurer, LONG, FULL, fit.font_px + 1), "{fit:?} is not the largest size");
}

#[wasm_bindgen_test]
async fn longer_lines_and_narrower_areas_get_smaller_text() {
    let measurer = stage(1.0);
    let theme = Theme::default();
    let short = fit_deck(&deck("short", &[SHORT]), &theme, FULL, &measurer).await;
    let long = fit_deck(&deck("long", &[LONG]), &theme, FULL, &measurer).await;
    assert!(long.font_px < short.font_px, "{long:?} vs {short:?}");
    // The same song next to an exclusion zone: less width, smaller text.
    let beside_zone = Rect::new(648, 48, 1224, 984);
    let narrow = fit_deck(&deck("long", &[LONG]), &theme, beside_zone, &measurer).await;
    assert!(narrow.font_px < long.font_px, "{narrow:?} vs {long:?}");
}

#[wasm_bindgen_test]
async fn one_size_for_the_whole_deck_is_set_by_its_longest_slide() {
    let measurer = stage(1.0);
    let theme = Theme::default();
    let alone = fit_deck(&deck("long-alone", &[LONG]), &theme, FULL, &measurer).await;
    let together = fit_deck(&deck("mixed", &[SHORT, LONG, SHORT]), &theme, FULL, &measurer).await;
    assert_eq!(together, alone);
}

#[wasm_bindgen_test]
async fn scaling_the_stage_does_not_change_the_size() {
    let theme = Theme::default();
    let screen = fit_deck(&deck("scale-1", &[SHORT, LONG]), &theme, FULL, &stage(1.0)).await;
    // A remote preview is the same stage scaled down; a new group avoids the cache.
    let preview = fit_deck(&deck("scale-0.3", &[SHORT, LONG]), &theme, FULL, &stage(0.3)).await;
    assert_eq!(preview, screen);
}
