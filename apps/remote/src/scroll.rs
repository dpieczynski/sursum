// SPDX-License-Identifier: GPL-3.0-or-later
//! Keeping the live slide tile and the live set row in view after the snapshot changed them.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Where the element should end up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Scroll as little as possible to show it.
    Nearest,
    /// Second from the top of its list, so the previous item stays visible above it.
    Second,
}

/// After the next frame (when Leptos has rendered the new state), scrolls the element matching
/// `selector` inside `container` into view.
pub fn reveal_after_render<E>(container: NodeRef<E>, selector: &'static str, place: Place)
where
    E: leptos::html::ElementType,
    E::Output: JsCast + Clone + 'static,
{
    request_animation_frame(move || {
        let Some(container) = container.get_untracked() else { return };
        let container: web_sys::Element = container.unchecked_into();
        let Ok(Some(target)) = container.query_selector(selector) else { return };
        match place {
            Place::Nearest => {
                let options = web_sys::ScrollIntoViewOptions::new();
                options.set_block(web_sys::ScrollLogicalPosition::Nearest);
                options.set_inline(web_sys::ScrollLogicalPosition::Nearest);
                target.scroll_into_view_with_scroll_into_view_options(&options);
            }
            Place::Second => {
                let target: web_sys::HtmlElement = target.unchecked_into();
                container.set_scroll_top((target.offset_top() - target.offset_height()).max(0));
            }
        }
    });
}
