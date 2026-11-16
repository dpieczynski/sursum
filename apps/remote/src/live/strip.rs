// SPDX-License-Identifier: GPL-3.0-or-later
//! The live strip at the bottom of the other views and the editor on narrow screens: what is on
//! screen in a few words, and "Wygaś ekran / Pokaż tekst", "Wstecz", "Dalej", so black is one tap
//! away wherever the operator is.

use leptos::prelude::*;
use sursum_protocol::RemoteState;

use crate::Commands;
use crate::live::info::{Shows, live_position, screen_shows};
use crate::live::pad::{CoverButton, NextButton, PrevButton};

#[component]
pub fn LiveStrip(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let summary = move || {
        state.with(|state| {
            let state = state.as_ref()?;
            Some(match screen_shows(state) {
                Shows::Text | Shows::Picture | Shows::Gap => {
                    live_position(state).unwrap_or_else(|| screen_shows(state).label().to_owned())
                }
                other => other.label().to_owned(),
            })
        })
    };
    view! {
        <div class="strip" data-testid="live-strip">
            <span class="strip-summary">{summary}</span>
            <CoverButton state commands size="btn--md" />
            <PrevButton state commands size="btn--md" />
            <NextButton state commands class="strip-next" />
        </div>
    }
}
