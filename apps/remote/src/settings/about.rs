// SPDX-License-Identifier: GPL-3.0-or-later
//! O programie: the version, program updates and the licences (GPL-3.0-or-later, no warranty;
//! third-party notices on `/licenses`).

use leptos::prelude::*;

use super::Section;
use crate::messages::{Msg, pl};

#[component]
pub fn AboutSection() -> impl IntoView {
    // The desktop program is updated by installing a new one, not from inside.
    let on_device =
        expect_context::<sursum_protocol::Installation>() == sursum_protocol::Installation::Screen;
    view! {
        <Section title=Msg::SectionAbout>
            <p class="hint">{format!("sursum · {} {}", pl(Msg::Version), sursum_client::BUILD_ID)}</p>
            <Show when=move || on_device>
                <super::update::Updates />
            </Show>
            <p class="hint">{pl(Msg::LicenseNote)}</p>
            <a class="btn btn--sm btn--quiet" href="/licenses">
                {pl(Msg::About)}
            </a>
        </Section>
    }
}
