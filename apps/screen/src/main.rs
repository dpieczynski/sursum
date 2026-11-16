// SPDX-License-Identifier: GPL-3.0-or-later
//! The audience screen. It only draws what the server sends; it has no UI and never shows an
//! error. If the WebAssembly panics, the page reloads itself (the black background in
//! `index.html` covers the reload).

// Leptos components take their props by value; clippy's pass-by-value lint does not fit them.
#![allow(clippy::needless_pass_by_value)]

use leptos::prelude::*;
use sursum_client::{Client, needs_reload};
use sursum_protocol::{Overlay, Role, ScreenState, ServerMsg};
use sursum_slide::LayeredStage;

fn main() {
    install_panic_hook();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let state = RwSignal::new(None::<ScreenState>);
    let reload_pending = StoredValue::new(false);

    let client = Client::connect(Role::Screen, move |message| match message {
        ServerMsg::Welcome { .. } if needs_reload(&message) => {
            reload_pending.set_value(true);
            reload_if_safe(state.get_untracked().as_ref());
        }
        ServerMsg::Screen { state: snapshot } => {
            if reload_pending.get_value() {
                // A new build is running: reload at a moment the congregation will not notice.
                reload_if_safe(Some(&snapshot));
            }
            state.set(Some(snapshot));
        }
        _ => {}
    });
    // The connection lives as long as the page.
    let _client = StoredValue::new_local(client);

    view! { <LayeredStage state=state /> }
}

/// Reloads now if nothing is visible to the congregation (no slide, or black/logo on top).
fn reload_if_safe(state: Option<&ScreenState>) {
    let safe = state.is_none_or(|state| state.live.is_none() || state.overlay != Overlay::None);
    if safe {
        reload();
    }
}

fn reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

/// Logs panics to the console and reloads the page after a short delay.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        // A panicked WebAssembly module cannot run Rust code any more, so the reload is
        // scheduled as plain JavaScript.
        let reload = js_sys::Function::new_no_args("location.reload()");
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&reload, 2_000);
        }
    }));
}
