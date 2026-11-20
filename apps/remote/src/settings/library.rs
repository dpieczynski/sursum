// SPDX-License-Identifier: GPL-3.0-or-later
//! Synchronizacja: the library file (songs, churches, every church's sets and their pictures,
//! ADR 0025) to save and to load on another device, and importing from OpenSong.

use leptos::prelude::*;

use super::Section;
use crate::messages::Msg;

#[component]
pub fn LibrarySection() -> impl IntoView {
    view! {
        <Section title=Msg::SectionLibrary>
            <div class="setting-row">
                <crate::bundle::ExportLink />
            </div>
            <div class="setting-row">
                <crate::bundle::MergeButton />
            </div>
            <div class="setting-row">
                <crate::import::ImportButton />
            </div>
        </Section>
    }
}
