// SPDX-License-Identifier: GPL-3.0-or-later
//! Admin mode: changes to the library as a whole and to the device's setup (import, merge,
//! calibration, logo, paired devices). A paired remote unlocks it by entering the PIN again; the
//! server ends it after a while and enforces it on every request, so the remote only decides what
//! to show. The tools themselves are in the Ustawienia view.

use leptos::prelude::*;
use sursum_protocol::{AdminRequest, DeviceView, PairOutcome, SessionInfo};

use crate::api::{get_json, post_json, request_status};
use crate::messages::{Msg, locked_for, pl};
use crate::ui::HoldButton;

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

/// Re-reads the session every minute while admin mode is on, so the admin tools hide when it
/// expires. Lives as long as the calling component.
pub fn watch_expiry(session: Session) {
    let refresh = gloo_timers::callback::Interval::new(REFRESH_MS, move || {
        if session.0.get_untracked().admin {
            session.refresh();
        }
    });
    // Dropped (and so cancelled) together with the component.
    let _refresh = StoredValue::new_local(refresh);
}

/// Ends admin mode on this device.
pub fn end_admin(session: Session) {
    leptos::task::spawn_local(async move {
        request_status("DELETE", "/api/admin").await;
        session.refresh();
    });
}

/// Asks for the PIN to unlock admin mode on this device.
#[component]
pub fn AdminUnlock(open: RwSignal<bool>, session: Session) -> impl IntoView {
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
        <crate::ui::Sheet
            title=pl(Msg::AdminMode).to_owned()
            on_close=move || open.set(false)
            testid="admin-unlock"
        >
            <form class="sheet-form" on:submit=submit>
                <p class="hint">{pl(Msg::AdminHint)}</p>
                <label class="field">
                    {pl(Msg::AdminPin)} <crate::ui::PinField value=pin autofocus=true />
                </label>
                <p class="status-text">{move || status.get()}</p>
                <div class="sheet-actions">
                    <button type="submit" class="btn btn--primary" data-testid="admin-unlock-submit">
                        {pl(Msg::Unlock)}
                    </button>
                </div>
            </form>
        </crate::ui::Sheet>
    }
}

/// Paired remotes; any but this one can be revoked with a hold.
#[component]
pub fn DeviceList() -> impl IntoView {
    let devices = RwSignal::new(Vec::<DeviceView>::new());
    let load = move || {
        leptos::task::spawn_local(async move {
            devices.set(get_json::<Vec<DeviceView>>("/api/devices").await.unwrap_or_default());
        });
    };
    load();
    let rows = move || devices.get().into_iter().map(|device| device_row(device, load)).collect_view();
    view! {
        <ul class="device-list" data-testid="devices">
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
