// SPDX-License-Identifier: GPL-3.0-or-later
//! The fixed 1920×1080 stage, scaled to fill its container while keeping the aspect ratio.
//! Scaling with `transform` never changes layout, so text inside lays out identically at any
//! size; that is what makes the remote preview match the screen.

use leptos::prelude::*;
use sursum_protocol::{SCENE_HEIGHT, SCENE_WIDTH};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

/// A frame that fills its container and holds a scaled stage. `stage_ref` gives access to the
/// stage element (used for measuring and test attributes).
#[component]
pub fn Stage(
    children: Children,
    #[prop(optional)] stage_ref: Option<NodeRef<leptos::html::Div>>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let frame = NodeRef::<leptos::html::Div>::new();
    let stage = stage_ref.unwrap_or_default();
    let transform = RwSignal::new(String::from("scale(1)"));

    Effect::new(move |_| {
        let Some(element) = frame.get() else { return };
        let element: web_sys::Element = element.unchecked_into();
        let update = {
            let element = element.clone();
            move || transform.set(scale_transform(element.client_width(), element.client_height()))
        };
        update();
        let callback = Closure::<dyn FnMut()>::new(update);
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&element);
            // Browser objects are not `Send`; Leptos keeps them in local storage for us.
            let observer = StoredValue::new_local(observer);
            on_cleanup(move || {
                let _ = observer.try_with_value(web_sys::ResizeObserver::disconnect);
            });
        }
        // The callback must live as long as the observer; the stage lives as long as the page.
        callback.forget();
    });

    view! {
        <div class=format!("sursum-frame {class}") node_ref=frame>
            <div class="sursum-stage" node_ref=stage style:transform=move || transform.get()>
                {children()}
            </div>
        </div>
    }
}

/// `translate(...) scale(...)` that fits the stage into a `width`×`height` frame, centred.
pub fn scale_transform(width: i32, height: i32) -> String {
    let scale = (f64::from(width) / f64::from(SCENE_WIDTH)).min(f64::from(height) / f64::from(SCENE_HEIGHT));
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let offset_x = (f64::from(width) - f64::from(SCENE_WIDTH) * scale) / 2.0;
    let offset_y = (f64::from(height) - f64::from(SCENE_HEIGHT) * scale) / 2.0;
    format!("translate({offset_x}px, {offset_y}px) scale({scale})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_hd_frame_is_not_scaled() {
        assert_eq!(scale_transform(1920, 1080), "translate(0px, 0px) scale(1)");
    }

    #[test]
    fn wider_frame_is_letterboxed_horizontally() {
        assert_eq!(scale_transform(2000, 540), "translate(520px, 0px) scale(0.5)");
    }
}
