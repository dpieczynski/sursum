// SPDX-License-Identifier: GPL-3.0-or-later
//! The audience screen. It only draws what the server sends; it has no UI and never shows an
//! error. If the WebAssembly panics, the page reloads itself (the black background in
//! `index.html` covers the reload). Every reload waits until the server answers: reloading while
//! it is down would leave the browser on its own error page.

// Leptos components take their props by value; clippy's pass-by-value lint does not fit them.
#![allow(clippy::needless_pass_by_value)]

use leptos::prelude::*;
use sursum_client::{Client, ServerChange, server_change};
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
        ServerMsg::Welcome { .. } => match server_change(&message) {
            // No snapshot follows on this connection, so there is no safer moment to wait for;
            // the reload happens over the black background of `index.html`.
            Some(ServerChange::Protocol) => reload(),
            Some(ServerChange::Build) => {
                reload_pending.set_value(true);
                reload_if_safe(state.get_untracked().as_ref());
            }
            None => {}
        },
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

/// Plain JavaScript (a panicked WebAssembly module cannot run Rust code any more): reloads once
/// `/healthz` answers, asking again every half second until then. Runs once per page however
/// often it is called (a new build asks on `Welcome` and again on the next snapshot), and gives
/// up on an answer after 2 s, so a server that accepts but hangs is asked again on time. A page
/// younger than 5 s first waits until it is 5 s old: a page that keeps disagreeing with its
/// server (a stale frontend in a development build, a panic at start) reloads at most every 5 s
/// instead of as fast as the server answers, while an update still reloads at once.
const RELOAD_WHEN_SERVER_ANSWERS: &str = "(function () { \
    if (window.sursumReloading) { return; } \
    window.sursumReloading = true; \
    setTimeout(function ask() { \
        var controller = new AbortController(); \
        var timer = setTimeout(function () { controller.abort(); }, 2000); \
        fetch('/healthz', { cache: 'no-store', signal: controller.signal }).then( \
            function (response) { \
                clearTimeout(timer); \
                if (response.ok) { location.reload(); } else { setTimeout(ask, 500); } \
            }, \
            function () { clearTimeout(timer); setTimeout(ask, 500); }); \
    }, Math.max(0, 5000 - performance.now())); })()";

fn reload() {
    let _ = js_sys::Function::new_no_args(RELOAD_WHEN_SERVER_ANSWERS).call0(&wasm_bindgen::JsValue::NULL);
}

/// Logs panics to the console and reloads the page after a short delay.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        let reload = js_sys::Function::new_no_args(RELOAD_WHEN_SERVER_ANSWERS);
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&reload, 2_000);
        }
    }));
}
