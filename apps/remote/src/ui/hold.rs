// SPDX-License-Identifier: GPL-3.0-or-later
//! Hold-to-confirm: destructive actions run only after the button is held for a moment, so an
//! accidental touch while playing does nothing. The whole button fills while it is held; letting
//! go too early says how it works instead of doing nothing silently.

use gloo_timers::callback::Timeout;
use leptos::prelude::*;

use crate::messages::{Msg, pl};
use crate::ui::Toasts;

/// How long the button must be held.
pub const HOLD_MS: u32 = 700;

/// A button that runs `on_hold` after being pressed for `hold_ms` (default [`HOLD_MS`]);
/// releasing earlier cancels. `class` adds size and colour (default: a small danger button).
#[component]
pub fn HoldButton(
    label: &'static str,
    on_hold: impl Fn() + Clone + 'static,
    #[prop(default = HOLD_MS)] hold_ms: u32,
    #[prop(default = "btn--sm btn--danger")] class: &'static str,
    #[prop(optional)] testid: &'static str,
) -> impl IntoView {
    let holding = RwSignal::new(false);
    let timer = StoredValue::new_local(None::<Timeout>);
    let toasts = use_context::<Toasts>();
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
        if holding.get_untracked()
            && let Some(toasts) = toasts
        {
            toasts.show(pl(Msg::HoldToConfirm));
        }
        holding.set(false);
        // Dropping the timeout cancels it.
        timer.set_value(None);
    };
    let leave = move |_| {
        holding.set(false);
        timer.set_value(None);
    };
    let swallow_click = |event: leptos::ev::MouseEvent| event.stop_propagation();
    view! {
        <button
            class=format!("btn hold {class}")
            class:holding=holding
            style=format!("--hold-ms:{hold_ms}ms")
            data-testid=testid
            on:pointerdown=start
            on:pointerup=cancel
            on:pointerleave=leave
            on:pointercancel=leave
            on:click=swallow_click
            on:contextmenu=|event| event.prevent_default()
        >
            {label}
        </button>
    }
}
