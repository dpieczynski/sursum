// SPDX-License-Identifier: GPL-3.0-or-later
//! Baza pieśni: a copy of the whole library (any paired remote), merging another church's file
//! and importing from OpenSong.

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
