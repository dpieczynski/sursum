// SPDX-License-Identifier: GPL-3.0-or-later
//! The Ustawienia view: everything that is set up once or rarely, in sections. Items that need
//! admin mode are always visible with a lock, and ask for the PIN where they are, so nobody has
//! to know where admin mode is switched on.

mod about;
mod device;
mod display;
mod library;
mod screen;

use leptos::prelude::*;
use sursum_protocol::RemoteState;

use crate::Commands;
use crate::admin::{AdminUnlock, DeviceList, Session, end_admin, watch_expiry};
use crate::messages::{Msg, pl};

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
    view! {
        <div class="settings-view" data-testid="settings-view">
            <Show when=move || admin() && with_pin()>
                <div class="admin-on" data-testid="admin-on">
                    <span>{pl(Msg::AdminOn)}</span>
                    <button class="btn btn--sm" data-testid="end-admin" on:click=move |_| end_admin(session)>
                        {pl(Msg::EndAdmin)}
                    </button>
                </div>
            </Show>
            <screen::ScreenSection state commands />
            <display::DisplaySection state commands />
            <device::DeviceSection />
            <library::LibrarySection />
            <Section title=Msg::Devices>
                <AdminOnly label=Msg::Devices>
                    <DeviceList />
                </AdminOnly>
            </Section>
            <about::AboutSection />
        </div>
        {unlock_sheet}
    }
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
