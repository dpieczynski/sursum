// SPDX-License-Identifier: GPL-3.0-or-later
//! A PIN entry with a "show" switch, so a mistyped digit can be seen before sending.

use leptos::prelude::*;

use crate::messages::{Msg, pl};

/// A numeric PIN field bound to `value`.
#[component]
pub fn PinField(
    value: RwSignal<String>,
    #[prop(default = "off")] autocomplete: &'static str,
    #[prop(optional)] autofocus: bool,
) -> impl IntoView {
    let visible = RwSignal::new(false);
    let kind = move || if visible.get() { "text" } else { "password" };
    let toggle_label = move || if visible.get() { pl(Msg::HidePin) } else { pl(Msg::ShowPin) };
    view! {
        <span class="pin-field">
            <input
                class="pin"
                type=kind
                inputmode="numeric"
                autocomplete=autocomplete
                autofocus=autofocus
                maxlength="12"
                prop:value=value
                on:input=move |event| value.set(event_target_value(&event))
            />
            <button
                type="button"
                class="btn btn--quiet btn--sm"
                aria-pressed=move || visible.get().to_string()
                on:click=move |_| visible.update(|on| *on = !*on)
            >
                {toggle_label}
            </button>
        </span>
    }
}
