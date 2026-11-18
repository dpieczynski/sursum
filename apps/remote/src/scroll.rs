// SPDX-License-Identifier: GPL-3.0-or-later
//! Keeping the live slide tile and the live set row in view after the snapshot changed them.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// After the next frame (when Leptos has rendered the new state), scrolls the element matching
/// `selector` inside `container` to the second place of its list: the previous item stays
/// visible before it (half of it in a row that scrolls sideways, like the slide tiles on a
/// phone) and the following ones after it, so the operator sees what comes next.
pub fn reveal_after_render<E>(container: NodeRef<E>, selector: &'static str)
where
    E: leptos::html::ElementType,
    E::Output: JsCast + Clone + 'static,
{
    request_animation_frame(move || {
        let Some(container) = container.get_untracked() else { return };
        let container: web_sys::Element = container.unchecked_into();
        let Ok(Some(target)) = container.query_selector(selector) else { return };
        let (frame, rect) = (container.get_bounding_client_rect(), target.get_bounding_client_rect());
        let sideways = container.scroll_width() > container.client_width()
            && container.scroll_height() <= container.client_height();
        if sideways {
            let left = rect.left() - frame.left() + f64::from(container.scroll_left());
            container.set_scroll_left((left - rect.width() / 2.0).max(0.0) as i32);
        } else {
            let top = rect.top() - frame.top() + f64::from(container.scroll_top());
            container.set_scroll_top((top - rect.height()).max(0.0) as i32);
        }
    });
}
