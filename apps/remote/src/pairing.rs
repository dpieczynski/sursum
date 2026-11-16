// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing this browser with the device PIN (once per tablet or PC), and setting the PIN on a
//! new device (the first remote does it; until then nobody may control the screen).

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
                Some(PairOutcome::Paired | PairOutcome::Unlocked) => reload(),
                Some(PairOutcome::WrongPin | PairOutcome::InvalidPin) => {
                    status.set(Some(pl(Msg::WrongPin).to_owned()));
                }
                // The PIN is set; pairing with it is what this form does anyway.
                Some(PairOutcome::AlreadySet) => status.set(Some(pl(Msg::PinAlreadySet).to_owned())),
                Some(PairOutcome::Locked { retry_after_s }) => status.set(Some(locked_for(retry_after_s))),
                None => status.set(Some(pl(Msg::Disconnected).to_owned())),
            }
            pin.set(String::new());
        });
    };
    view! {
        <form class="pairing" on:submit=submit>
            <h1>{pl(Msg::PairingTitle)}</h1>
            <label>{pl(Msg::Pin)} <crate::ui::PinField value=pin autofocus=true /></label>
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

/// The form shown on a new device: choose the PIN (twice) and pair this browser with it.
#[component]
pub fn Setup() -> impl IntoView {
    let pin = RwSignal::new(String::new());
    let repeat = RwSignal::new(String::new());
    let name = RwSignal::new(pl(Msg::DefaultDeviceName).to_owned());
    let status = RwSignal::new(None::<&'static str>);
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if pin.get_untracked() != repeat.get_untracked() {
            status.set(Some(pl(Msg::PinsDiffer)));
            repeat.set(String::new());
            return;
        }
        let request = PairRequest { pin: pin.get_untracked(), name: name.get_untracked() };
        leptos::task::spawn_local(async move {
            match post_json::<_, PairOutcome>("/api/setup", &request).await {
                Some(PairOutcome::Paired) => reload(),
                Some(PairOutcome::AlreadySet) => reload(),
                Some(PairOutcome::InvalidPin) => status.set(Some(pl(Msg::InvalidPin))),
                Some(_) | None => status.set(Some(pl(Msg::Disconnected))),
            }
        });
    };
    view! {
        <form class="pairing" data-testid="setup" on:submit=submit>
            <h1>{pl(Msg::SetupTitle)}</h1>
            <p>{pl(Msg::SetupHint)}</p>
            <label>
                {pl(Msg::Pin)} <crate::ui::PinField value=pin autocomplete="new-password" autofocus=true />
            </label>
            <label>
                {pl(Msg::RepeatPin)} <crate::ui::PinField value=repeat autocomplete="new-password" />
            </label>
            <label>
                {pl(Msg::DeviceName)}
                <input prop:value=name on:input=move |event| name.set(event_target_value(&event)) />
            </label>
            <button class="primary" type="submit">
                {pl(Msg::SetPin)}
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
