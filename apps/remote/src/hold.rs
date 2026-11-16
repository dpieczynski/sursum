// SPDX-License-Identifier: GPL-3.0-or-later
//! Hold-to-confirm: destructive actions run only after the button is held for a moment, so an
//! accidental touch while playing does nothing.

use gloo_timers::callback::Timeout;
use leptos::prelude::*;

/// How long the button must be held.
pub const HOLD_MS: u32 = 700;

/// A button that runs `on_hold` after being pressed for `hold_ms` (default [`HOLD_MS`]);
/// releasing earlier cancels.
#[component]
pub fn HoldButton(
    label: &'static str,
    on_hold: impl Fn() + Clone + 'static,
    #[prop(default = HOLD_MS)] hold_ms: u32,
) -> impl IntoView {
    let holding = RwSignal::new(false);
    let timer = StoredValue::new_local(None::<Timeout>);
    let start = move |event: leptos::ev::PointerEvent| {
        event.stop_propagation();
        holding.set(true);
        let on_hold = on_hold.clone();
        let timeout = Timeout::new(hold_ms, move || {
            holding.set(false);
            on_hold();
        });
        timer.set_value(Some(timeout));
    };
    let cancel = move |_| {
        holding.set(false);
        // Dropping the timeout cancels it.
        timer.set_value(None);
    };
    let swallow_click = |event: leptos::ev::MouseEvent| event.stop_propagation();
    view! {
        <button
            class="row-button hold"
            class:holding=holding
            style=format!("--hold-ms:{hold_ms}ms")
            on:pointerdown=start
            on:pointerup=cancel
            on:pointerleave=cancel
            on:pointercancel=cancel
            on:click=swallow_click
        >
            {label}
        </button>
    }
}
