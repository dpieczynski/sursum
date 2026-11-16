// SPDX-License-Identifier: GPL-3.0-or-later
//! Choosing the font size: one size for a whole deck (the longest verse decides), so the text
//! does not jump between verses.
//!
//! The search itself is pure ([`fit_font_size`]); only the measuring closure touches the DOM
//! ([`fit_deck`]). Lines come from the server and are never re-flowed: the fitter looks for the
//! largest size at which every line fits on one row. Only if even the minimum size does not fit
//! is wrapping allowed.

use std::cell::RefCell;
use std::collections::HashMap;

use sursum_protocol::{Deck, Rect, SlideBody, Theme};
use wasm_bindgen::JsCast;

use crate::dom;
use crate::slide_view::{lines_class, text_box_style};

/// The chosen size for a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    pub font_px: u32,
    /// True when even `min_font_px` was too large and lines must wrap.
    pub wrap: bool,
}

/// The largest size in `[min, max]` for which `fits` holds, by binary search.
///
/// `fits` must be monotonic (if a size fits, every smaller size fits). Returns `None` when even
/// `min` does not fit.
pub fn fit_font_size(min: u32, max: u32, mut fits: impl FnMut(u32) -> bool) -> Option<u32> {
    let (mut low, mut high) = (min, max.max(min));
    if !fits(low) {
        return None;
    }
    // Invariant: `low` fits; the answer is in `[low, high]`.
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Some(low)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FitKey {
    fit_group: String,
    font: (String, u16, u16),
    size_range: (u32, u32),
    content: (u32, u32),
}

thread_local! {
    static CACHE: RefCell<HashMap<FitKey, Fit>> = RefCell::new(HashMap::new());
}

/// Fits a deck into the content rectangle, using `measurer` (an element inside a stage with the
/// `sursum-measure` class) to measure text. Results are cached per deck, theme and rectangle.
///
/// A measurer that is not laid out (inside `display: none`, e.g. a preview hidden on a narrow
/// layout) has no size, so everything would seem to fit: then the smallest size is returned and
/// nothing is cached, so a hidden preview never spoils the size of a visible one.
pub async fn fit_deck(deck: &Deck, theme: &Theme, content: Rect, measurer: &web_sys::HtmlElement) -> Fit {
    if measurer.client_width() == 0 || measurer.client_height() == 0 {
        return Fit { font_px: theme.min_font_px, wrap: false };
    }
    let key = FitKey {
        fit_group: deck.fit_group.clone(),
        font: (theme.font_family.clone(), theme.font_weight, theme.line_height_permille),
        size_range: (theme.min_font_px, theme.max_font_px),
        content: (content.width, content.height),
    };
    if let Some(fit) = CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        return fit;
    }
    dom::fonts_ready(theme).await;
    let fit = measure(deck, theme, content, measurer);
    CACHE.with(|cache| cache.borrow_mut().insert(key, fit));
    fit
}

/// Builds hidden text boxes for every text slide and searches for the largest size that fits.
fn measure(deck: &Deck, theme: &Theme, content: Rect, measurer: &web_sys::HtmlElement) -> Fit {
    let boxes: Vec<web_sys::HtmlElement> = deck
        .slides
        .iter()
        .filter_map(|slide| match &slide.body {
            SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => Some(lines),
            SlideBody::Image { .. } => None,
        })
        .filter_map(|lines| text_box(measurer, theme, lines))
        .collect();
    let fits = |size: u32| {
        boxes.iter().all(|element| {
            set_style(element, &text_box_style(theme, Rect::new(0, 0, content.width, content.height), size));
            element.scroll_width() <= element.client_width()
                && element.scroll_height() <= element.client_height()
        })
    };
    let fit = match fit_font_size(theme.min_font_px, theme.max_font_px, fits) {
        Some(font_px) => Fit { font_px, wrap: false },
        None => Fit { font_px: theme.min_font_px, wrap: true },
    };
    for element in boxes {
        element.remove();
    }
    fit
}

/// Creates one hidden text box with the same markup as [`crate::slide_view::SlideView`].
fn text_box(
    measurer: &web_sys::HtmlElement,
    theme: &Theme,
    lines: &[String],
) -> Option<web_sys::HtmlElement> {
    let document = web_sys::window()?.document()?;
    let container: web_sys::HtmlElement = document.create_element("div").ok()?.dyn_into().ok()?;
    container.set_class_name(&lines_class(theme, false));
    for line in lines {
        let row = document.create_element("div").ok()?;
        row.set_class_name("sursum-line");
        row.set_text_content(Some(line));
        container.append_child(&row).ok()?;
    }
    measurer.append_child(&container).ok()?;
    Some(container)
}

fn set_style(element: &web_sys::HtmlElement, style: &str) {
    let _ = element.set_attribute("style", style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_largest_fitting_size() {
        assert_eq!(fit_font_size(10, 100, |size| size <= 57), Some(57));
    }

    #[test]
    fn returns_max_when_everything_fits() {
        assert_eq!(fit_font_size(10, 100, |_| true), Some(100));
    }

    #[test]
    fn returns_none_when_even_min_is_too_large() {
        assert_eq!(fit_font_size(10, 100, |size| size < 10), None);
    }

    #[test]
    fn handles_inverted_range() {
        assert_eq!(fit_font_size(40, 20, |_| true), Some(40));
    }

    #[cfg(not(target_arch = "wasm32"))]
    proptest::proptest! {
        #[test]
        fn matches_a_linear_scan(min in 1u32..100, span in 0u32..200, limit in 0u32..400) {
            let max = min + span;
            let expected = (min..=max).rev().find(|size| *size <= limit);
            proptest::prop_assert_eq!(fit_font_size(min, max, |size| size <= limit), expected);
        }
    }
}
