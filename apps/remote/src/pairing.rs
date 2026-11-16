// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing this browser with the device PIN (once per tablet or PC).

use leptos::prelude::*;
use sursum_protocol::{PairOutcome, PairRequest};

use crate::api::post_json;
use crate::messages::{Msg, locked_for, pl};

/// The PIN form shown instead of the remote until the browser is paired.
#[component]
pub fn Pairing() -> impl IntoView {
    let pin = RwSignal::new(String::new());
    let name = RwSignal::new(pl(Msg::DefaultDeviceName).to_owned());
    let status = RwSignal::new(None::<String>);
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let request = PairRequest { pin: pin.get_untracked(), name: name.get_untracked() };
        leptos::task::spawn_local(async move {
            match post_json::<_, PairOutcome>("/api/pair", &request).await {
                Some(PairOutcome::Paired) => reload(),
                Some(PairOutcome::WrongPin) => status.set(Some(pl(Msg::WrongPin).to_owned())),
                Some(PairOutcome::Locked { retry_after_s }) => status.set(Some(locked_for(retry_after_s))),
                None => status.set(Some(pl(Msg::Disconnected).to_owned())),
            }
            pin.set(String::new());
        });
    };
    view! {
        <form class="pairing" on:submit=submit>
            <h1>{pl(Msg::PairingTitle)}</h1>
            <label>
                {pl(Msg::Pin)}
                <input
                    class="pin"
                    type="password"
                    inputmode="numeric"
                    autocomplete="off"
                    prop:value=pin
                    on:input=move |event| pin.set(event_target_value(&event))
                />
            </label>
            <label>
                {pl(Msg::DeviceName)}
                <input prop:value=name on:input=move |event| name.set(event_target_value(&event)) />
            </label>
            <button class="primary" type="submit">
                {pl(Msg::Pair)}
            </button>
            <p class="editor-status">{move || status.get()}</p>
        </form>
    }
}

fn reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}
