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
    let name = RwSignal::new(suggested_name());
    let status = RwSignal::new(None::<String>);
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let request = PairRequest { pin: pin.get_untracked(), name: name.get_untracked() };
        leptos::task::spawn_local(async move {
            match post_json::<_, PairOutcome>("/api/pair", &request).await {
                Some(PairOutcome::Paired) => reload(),
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
    let name = RwSignal::new(suggested_name());
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

/// A name that tells paired devices apart in the device list: "Tablet", "Telefon" or
/// "Komputer (Windows)" from the browser's description of itself; the operator can change it.
fn suggested_name() -> String {
    let user_agent = web_sys::window().and_then(|window| window.navigator().user_agent().ok());
    device_name(user_agent.as_deref().unwrap_or_default())
}

/// The suggested name for a browser with `user_agent`.
fn device_name(user_agent: &str) -> String {
    let has = |part: &str| user_agent.contains(part);
    let computer = |system: &str| format!("{} ({system})", pl(Msg::DeviceComputer));
    if has("iPhone") || (has("Android") && has("Mobile")) {
        pl(Msg::DevicePhone).to_owned()
    } else if has("Android") || has("iPad") {
        pl(Msg::DeviceTablet).to_owned()
    } else if has("Windows") {
        computer("Windows")
    } else if has("Macintosh") {
        computer("Mac")
    } else if has("Linux") {
        computer("Linux")
    } else {
        pl(Msg::DefaultDeviceName).to_owned()
    }
}

fn reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_names_tell_devices_apart() {
        let tablet = "Mozilla/5.0 (Linux; Android 14; SM-X200) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";
        let phone =
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/131.0 Mobile Safari/537.36";
        let windows =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";
        let linux = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";
        assert_eq!(device_name(tablet), "Tablet");
        assert_eq!(device_name(phone), "Telefon");
        assert_eq!(device_name(windows), "Komputer (Windows)");
        assert_eq!(device_name(linux), "Komputer (Linux)");
        assert_eq!(device_name(""), "Pilot");
    }
}
