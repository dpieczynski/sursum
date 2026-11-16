// SPDX-License-Identifier: GPL-3.0-or-later
//! Baza pieśni: a copy of the whole library (any paired remote), merging another church's file
//! and importing from OpenSong (admin mode).

use leptos::prelude::*;

use super::{AdminOnly, Section};
use crate::messages::Msg;

#[component]
pub fn LibrarySection() -> impl IntoView {
    view! {
        <Section title=Msg::SectionLibrary>
            <div class="setting-row">
                <crate::bundle::ExportLink />
            </div>
            <div class="setting-row">
                <AdminOnly label=Msg::MergeLibrary>
                    <crate::bundle::MergeButton />
                </AdminOnly>
            </div>
            <div class="setting-row">
                <AdminOnly label=Msg::ImportOpenSong>
                    <crate::import::ImportButton />
                </AdminOnly>
            </div>
        </Section>
    }
}
