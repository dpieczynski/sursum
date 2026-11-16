// SPDX-License-Identifier: GPL-3.0-or-later
//! A sheet: a window with a title and a close button over a dimmed page. Full screen on phones,
//! a card on wider screens. Esc and a tap on the dimmed area close it. Pedal keys keep working
//! while it is open, so black stays one press away.

use leptos::ev;
use leptos::prelude::*;

use crate::messages::{Msg, pl};

/// A sheet with `title`; `on_close` runs when it is closed (button, Esc, tap outside).
#[component]
pub fn Sheet(
    title: String,
    on_close: impl Fn() + Copy + Send + Sync + 'static,
    #[prop(optional)] testid: &'static str,
    children: Children,
) -> impl IntoView {
    let escape = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            on_close();
        }
    });
    on_cleanup(move || escape.remove());
    let backdrop = move |event: ev::MouseEvent| {
        use wasm_bindgen::JsCast;
        let on_backdrop = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|element| element.class_list().contains("sheet-backdrop"));
        if on_backdrop {
            on_close();
        }
    };
    let label = title.clone();
    view! {
        <div class="sheet-backdrop" on:click=backdrop>
            <div class="sheet" role="dialog" aria-modal="true" aria-label=label data-testid=testid>
                <header class="sheet-header">
                    <h2>{title}</h2>
                    <button class="btn btn--quiet btn--sm" on:click=move |_| on_close()>
                        {pl(Msg::Close)}
                    </button>
                </header>
                <div class="sheet-body">{children()}</div>
            </div>
        </div>
    }
}
