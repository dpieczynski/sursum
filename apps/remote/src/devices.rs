// SPDX-License-Identifier: GPL-3.0-or-later
//! The list of paired remotes (Ustawienia → Urządzenia): a lost or foreign tablet is revoked with
//! a hold and must pair with the PIN again. Any paired remote may do this (ADR 0024).

use leptos::prelude::*;
use sursum_protocol::DeviceView;

use crate::api::{get_json, request_status};
use crate::messages::{Msg, paired_on, pl};
use crate::ui::HoldButton;

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
            <span class="device-date">{paired_label(&device.paired_at)}</span>
            {action}
        </li>
    }
}

/// "2026-11-18 00:15:56" (UTC, as the database stores it) → "sparowano 18.11.2026, 1:15" in this
/// browser's local time; anything unexpected is shown as it came.
fn paired_label(paired_at: &str) -> String {
    let utc = format!("{}Z", paired_at.trim().replacen(' ', "T", 1));
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&utc));
    if date.get_time().is_nan() {
        return paired_at.to_owned();
    }
    paired_on([
        date.get_date(),
        date.get_month() + 1,
        date.get_full_year(),
        date.get_hours(),
        date.get_minutes(),
    ])
}
