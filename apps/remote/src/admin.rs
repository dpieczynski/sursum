// SPDX-License-Identifier: GPL-3.0-or-later
//! Admin mode: changes to the library as a whole and to the device's setup (import, merge,
//! calibration, paired devices). A paired remote unlocks it by entering the PIN again; the server
//! ends it after a while and enforces it on every request, so this module only decides what to
//! show.

use leptos::prelude::*;
use sursum_protocol::{AdminRequest, DeviceView, PairOutcome, SessionInfo};

use crate::api::{get_json, post_json, request_status};
use crate::calibration::Calibrating;
use crate::hold::HoldButton;
use crate::messages::{Msg, locked_for, pl};

/// While admin mode is on, the session is re-read this often so the tools hide when it expires.
const REFRESH_MS: u32 = 60_000;

/// What the server said about this browser (shared through the Leptos context).
#[derive(Clone, Copy)]
pub struct Session(pub RwSignal<SessionInfo>);

impl Session {
    /// Asks the server again (after admin mode may have expired or changed).
    pub fn refresh(self) {
        leptos::task::spawn_local(async move {
            if let Some(info) = get_json::<SessionInfo>("/api/session").await {
                self.0.set(info);
            }
        });
    }
}

/// The admin tools, or the button that unlocks them.
#[component]
pub fn AdminTools() -> impl IntoView {
    let session = expect_context::<Session>();
    let unlocking = RwSignal::new(false);
    let devices = RwSignal::new(false);
    let calibrating = expect_context::<Calibrating>();
    let commands = expect_context::<crate::Commands>();
    let refresh = gloo_timers::callback::Interval::new(REFRESH_MS, move || {
        if session.0.get_untracked().admin {
            session.refresh();
        }
    });
    // Dropped (and so cancelled) together with this component.
    let _refresh = StoredValue::new_local(refresh);
    let admin = move || session.0.get().admin;
    let with_pin = move || session.0.get().pin_required;
    let end = move |_| {
        leptos::task::spawn_local(async move {
            request_status("DELETE", "/api/admin").await;
            session.refresh();
        });
        devices.set(false);
    };
    let tools = move || {
        view! {
            <crate::import::ImportButton />
            <crate::bundle::MergeButton />
            <HoldButton label=pl(Msg::Calibrate) on_hold=move || calibrating.0.set(true) />
            <crate::picture::LogoButton commands />
            <Show when=with_pin>
                <button class="import-button" on:click=move |_| devices.update(|open| *open = !*open)>
                    {pl(Msg::Devices)}
                </button>
                <button class="import-button" on:click=end>
                    {pl(Msg::EndAdmin)}
                </button>
            </Show>
        }
    };
    let locked = move || {
        view! {
            <button class="import-button" on:click=move |_| unlocking.set(true)>
                {pl(Msg::AdminMode)}
            </button>
        }
    };
    view! {
        <Show when=admin fallback=locked>
            {tools}
        </Show>
        <Show when=move || unlocking.get() && !admin()>
            <AdminUnlock open=unlocking session />
        </Show>
        <Show when=move || devices.get() && admin()>
            <DeviceList />
        </Show>
    }
}

/// Asks for the PIN to unlock admin mode on this device.
#[component]
fn AdminUnlock(open: RwSignal<bool>, session: Session) -> impl IntoView {
    let pin = RwSignal::new(String::new());
    let status = RwSignal::new(None::<String>);
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let request = AdminRequest { pin: pin.get_untracked() };
        leptos::task::spawn_local(async move {
            match post_json::<_, PairOutcome>("/api/admin", &request).await {
                Some(PairOutcome::Unlocked | PairOutcome::Paired) => {
                    open.set(false);
                    session.refresh();
                }
                Some(PairOutcome::WrongPin | PairOutcome::InvalidPin) => {
                    status.set(Some(pl(Msg::WrongPin).to_owned()));
                }
                Some(PairOutcome::Locked { retry_after_s }) => status.set(Some(locked_for(retry_after_s))),
                Some(PairOutcome::AlreadySet) | None => status.set(Some(pl(Msg::Disconnected).to_owned())),
            }
            pin.set(String::new());
        });
    };
    view! {
        <form class="dialog" on:submit=submit>
            <label>
                {pl(Msg::AdminPin)}
                <input
                    class="pin"
                    type="password"
                    inputmode="numeric"
                    autocomplete="off"
                    prop:value=pin
                    on:input=move |event| pin.set(event_target_value(&event))
                />
            </label>
            <p class="editor-status">{move || status.get()}</p>
            <div class="editor-buttons">
                <button type="button" on:click=move |_| open.set(false)>
                    {pl(Msg::Cancel)}
                </button>
                <button type="submit" class="primary">
                    {pl(Msg::Unlock)}
                </button>
            </div>
        </form>
    }
}

/// Paired remotes; any but this one can be revoked with a hold.
#[component]
fn DeviceList() -> impl IntoView {
    let devices = RwSignal::new(Vec::<DeviceView>::new());
    let load = move || {
        leptos::task::spawn_local(async move {
            devices.set(get_json::<Vec<DeviceView>>("/api/devices").await.unwrap_or_default());
        });
    };
    load();
    let rows = move || devices.get().into_iter().map(|device| device_row(device, load)).collect_view();
    view! {
        <ul class="dialog device-list" data-testid="devices">
            {rows}
        </ul>
    }
}

fn device_row(device: DeviceView, reload: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
    let revoke = {
        let path = format!("/api/devices/{}/revoke", device.device_id);
        move || {
            let path = path.clone();
            leptos::task::spawn_local(async move {
                request_status("POST", &path).await;
                reload();
            });
        }
    };
    let action = if device.this_device {
        view! { <span class="device-self">{pl(Msg::ThisDevice)}</span> }.into_any()
    } else {
        view! { <HoldButton label=pl(Msg::Revoke) on_hold=revoke /> }.into_any()
    };
    view! {
        <li class="device">
            <span class="device-name">{device.name}</span>
            <span class="device-date">{device.paired_at}</span>
            {action}
        </li>
    }
}
