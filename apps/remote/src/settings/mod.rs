// SPDX-License-Identifier: GPL-3.0-or-later
//! The Ustawienia view: everything that is set up once or rarely, in sections. Items that need
//! admin mode are always visible with a lock, and ask for the PIN where they are, so nobody has
//! to know where admin mode is switched on.

mod about;
mod backup;
mod device;
mod display;
mod library;
mod pictures;
mod screen;
mod update;

use leptos::prelude::*;
use sursum_protocol::{Installation, RemoteState};

use crate::Commands;
use crate::admin::{AdminUnlock, DeviceList, Session, end_admin, watch_expiry};
use crate::messages::{Msg, admin_minutes_left, pl};

/// Asking for the PIN from anywhere in the view (shared through the context).
#[derive(Clone, Copy)]
struct Unlock(RwSignal<bool>);

#[component]
pub fn SettingsView(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let session = expect_context::<Session>();
    watch_expiry(session);
    let unlocking = RwSignal::new(false);
    provide_context(Unlock(unlocking));
    let admin = move || session.0.get().admin;
    let with_pin = move || session.0.get().pin_required;
    let unlock_sheet =
        move || (unlocking.get() && !admin()).then(|| view! { <AdminUnlock open=unlocking session /> });
    // The desktop program's library has no screen, no remotes to pair and no device to update.
    let has_screen = expect_context::<Installation>() == Installation::Screen;
    let device_sections = move || {
        view! {
            <screen::ScreenSection state commands />
            <display::DisplaySection state commands />
            <device::DeviceSection />
        }
    };
    let devices = move || {
        view! {
            <Section title=Msg::Devices>
                <AdminOnly label=Msg::ShowDevices>
                    <DeviceList />
                </AdminOnly>
            </Section>
        }
    };
    view! {
        <div class="settings-view" data-testid="settings-view">
            <Show when=move || admin() && with_pin()>
                <AdminOn session />
            </Show>
            <Show when=move || has_screen>{device_sections}</Show>
            <library::LibrarySection />
            <backup::BackupSection />
            <Show when=move || has_screen>{devices}</Show>
            <about::AboutSection />
        </div>
        {unlock_sheet}
    }
}

/// "Admin mode ends in 14 min" with a button to end it now. Counts down locally between
/// session reads and asks the server again when the time is up, so the locks come back.
#[component]
fn AdminOn(session: Session) -> impl IntoView {
    let read_at = RwSignal::new(js_sys::Date::now());
    Effect::new(move |_| {
        session.0.track();
        read_at.set(js_sys::Date::now());
    });
    let now = RwSignal::new(js_sys::Date::now());
    let tick = gloo_timers::callback::Interval::new(10_000, move || now.set(js_sys::Date::now()));
    // Dropped (and so cancelled) together with the component.
    let _tick = StoredValue::new_local(tick);
    let minutes = move || {
        let remaining = session.0.get().admin_remaining_s?;
        Some(minutes_left(remaining, now.get() - read_at.get()))
    };
    Effect::new(move |_| {
        if minutes() == Some(0) {
            session.refresh();
        }
    });
    let text = move || minutes().map_or_else(|| pl(Msg::AdminOn).to_owned(), admin_minutes_left);
    view! {
        <div class="admin-on" data-testid="admin-on">
            <span>{text}</span>
            <button class="btn btn--sm" data-testid="end-admin" on:click=move |_| end_admin(session)>
                {pl(Msg::EndAdmin)}
            </button>
        </div>
    }
}

/// Whole minutes still left (rounded up) after `elapsed_ms` since the server said `remaining_s`.
fn minutes_left(remaining_s: u32, elapsed_ms: f64) -> u32 {
    // The clock ticks separately from the session read, so it may lag behind it a little.
    let left_s = f64::from(remaining_s) - elapsed_ms.max(0.0) / 1000.0;
    if left_s <= 0.0 { 0 } else { (left_s / 60.0).ceil() as u32 }
}

/// One card with a heading.
#[component]
fn Section(title: Msg, children: Children) -> impl IntoView {
    view! {
        <section class="card">
            <h2 class="card-heading">{pl(title)}</h2>
            {children()}
        </section>
    }
}

/// Shows `children` in admin mode; otherwise a locked button that asks for the PIN.
#[component]
fn AdminOnly(label: Msg, children: ChildrenFn) -> impl IntoView {
    let session = expect_context::<Session>();
    let unlock = expect_context::<Unlock>();
    let locked = move || {
        view! {
            <button class="btn btn--quiet locked" data-testid="locked" on:click=move |_| unlock.0.set(true)>
                <svg class="lock-icon" viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M7 10V7a5 5 0 0 1 10 0v3" fill="none" stroke="currentColor" stroke-width="2.4" />
                    <rect x="4" y="10" width="16" height="11" rx="2" fill="currentColor" />
                </svg>
                {pl(label)}
                <span class="locked-hint">{pl(Msg::NeedsAdmin)}</span>
            </button>
        }
    };
    view! {
        <Show when=move || session.0.get().admin fallback=locked>
            {children()}
        </Show>
    }
}
