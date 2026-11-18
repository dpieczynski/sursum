// SPDX-License-Identifier: GPL-3.0-or-later
//! A sheet: a window with a title and a close button over a dimmed page. Full screen on phones,
//! a card on wider screens. Esc and a tap on the dimmed area close it. The live controls (the
//! pad, or the live strip) are left out of the dimmed area and stay usable, so black is one tap
//! away while a sheet is open too (ADR 0017); pedal keys keep working as well.

use leptos::ev;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::messages::{Msg, pl};

/// A sheet with `title`; `on_close` runs when it is closed (button, Esc, tap outside). While
/// `locked` is true it cannot be closed, has no close button and covers the controls too (e.g.
/// while the device writes a system update and restarts: nothing else should look usable).
#[component]
pub fn Sheet(
    title: String,
    on_close: impl Fn() + Copy + Send + Sync + 'static,
    #[prop(optional)] testid: &'static str,
    #[prop(optional, into)] locked: MaybeProp<bool>,
    children: Children,
) -> impl IntoView {
    let locked = move || locked.get().unwrap_or(false);
    let on_close = move || {
        if !locked() {
            on_close();
        }
    };
    let escape = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            on_close();
        }
    });
    on_cleanup(move || escape.remove());
    let backdrop = move |event: ev::MouseEvent| {
        let on_backdrop = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|element| element.class_list().contains("sheet-backdrop"));
        if on_backdrop {
            on_close();
        }
    };
    let free = RwSignal::new(String::new());
    request_animation_frame(move || free.set(controls_free_style()));
    let resize = window_event_listener(ev::resize, move |_| free.set(controls_free_style()));
    on_cleanup(move || resize.remove());
    let style = move || if locked() { String::new() } else { free.get() };
    let sheet = NodeRef::<Div>::new();
    focus_first_field(sheet);
    let label = title.clone();
    view! {
        <div class="sheet-backdrop" style=style on:click=backdrop>
            <div
                class="sheet"
                role="dialog"
                aria-modal="true"
                aria-label=label
                data-testid=testid
                node_ref=sheet
            >
                <header class="sheet-header">
                    <h2>{title}</h2>
                    <Show when=move || !locked()>
                        <button class="btn btn--quiet btn--sm" on:click=move |_| on_close()>
                            {pl(Msg::Close)}
                        </button>
                    </Show>
                </header>
                <div class="sheet-body">{children()}</div>
            </div>
        </div>
    }
}

/// The backdrop's style that leaves the visible live controls (`.area-pad` beside or below the
/// content, `.area-strip` below it) undimmed and tappable: a hole cut exactly around them, and
/// padding on their side so the sheet is centred in the rest. Empty if there are none.
fn controls_free_style() -> String {
    let Some(window) = web_sys::window() else { return String::new() };
    let Some(document) = window.document() else { return String::new() };
    let width = window.inner_width().ok().and_then(|value| value.as_f64()).unwrap_or_default();
    let height = window.inner_height().ok().and_then(|value| value.as_f64()).unwrap_or_default();
    let Ok(regions) = document.query_selector_all(".area-pad, .area-strip") else { return String::new() };
    let visible = (0..regions.length())
        .filter_map(|index| regions.item(index)?.dyn_into::<web_sys::Element>().ok())
        .map(|region| region.get_bounding_client_rect())
        .find(|rect| rect.width() > 0.0 && rect.height() > 0.0);
    let Some(rect) = visible else { return String::new() };
    let padding = if rect.left() >= width / 2.0 {
        format!("padding-right:{}px", (width - rect.left()).ceil())
    } else if rect.right() <= width / 2.0 {
        format!("padding-left:{}px", rect.right().ceil())
    } else {
        format!("padding-bottom:{}px", (height - rect.top()).ceil())
    };
    let (left, top, right, bottom) =
        (rect.left().floor(), rect.top().floor(), rect.right().ceil(), rect.bottom().ceil());
    // One path around the window and one around the controls; with even-odd filling the inner
    // one is a hole, and a click there reaches the controls.
    format!(
        "{padding};clip-path:polygon(evenodd,0 0,100% 0,100% 100%,0 100%,0 0,\
         {left}px {top}px,{right}px {top}px,{right}px {bottom}px,{left}px {bottom}px,{left}px {top}px)"
    )
}

/// Puts the cursor in the sheet's field marked `autofocus` once it is shown (the attribute
/// itself only works for fields present when the page loads).
fn focus_first_field(sheet: NodeRef<Div>) {
    Effect::new(move |_| {
        let Some(sheet) = sheet.get() else { return };
        request_animation_frame(move || {
            let Ok(Some(field)) = sheet.query_selector("[autofocus]") else { return };
            if let Ok(field) = field.dyn_into::<web_sys::HtmlElement>() {
                let _ = field.focus();
            }
        });
    });
}
