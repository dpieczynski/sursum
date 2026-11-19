// SPDX-License-Identifier: GPL-3.0-or-later
//! The Ustawienia view: everything that is set up once or rarely, in sections. A paired remote
//! may use all of it (ADR 0024).

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
use crate::devices::DeviceList;
use crate::messages::{Msg, pl};

#[component]
pub fn SettingsView(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
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
                <DeviceList />
            </Section>
        }
    };
    view! {
        <div class="settings-view" data-testid="settings-view">
            <Show when=move || has_screen>{device_sections}</Show>
            <library::LibrarySection />
            <backup::BackupSection />
            <Show when=move || has_screen>{devices}</Show>
            <about::AboutSection />
        </div>
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
