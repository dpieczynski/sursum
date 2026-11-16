// SPDX-License-Identifier: GPL-3.0-or-later
//! Wyświetlanie: what "Dalej" does after a song's last slide (a setting of the device, so every
//! remote behaves the same).

use leptos::prelude::*;
use sursum_protocol::{Command, EndOfItem, RemoteState};

use super::Section;
use crate::Commands;
use crate::messages::{Msg, pl};

#[component]
pub fn DisplaySection(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let current =
        move || state.with(|state| state.as_ref().map(|state| state.end_of_item).unwrap_or_default());
    let option = move |mode: EndOfItem, label: Msg, testid: &'static str| {
        view! {
            <label class="choice">
                <input
                    type="radio"
                    name="end-of-item"
                    data-testid=testid
                    prop:checked=move || current() == mode
                    on:change=move |_| commands.send(Command::SetEndOfItem { mode })
                />
                {pl(label)}
            </label>
        }
    };
    view! {
        <Section title=Msg::SectionDisplay>
            <fieldset class="choices">
                <legend>{pl(Msg::EndOfItemLabel)}</legend>
                {option(EndOfItem::Gap, Msg::EndGap, "end-gap")}
                {option(EndOfItem::Continue, Msg::EndContinue, "end-continue")}
            </fieldset>
        </Section>
    }
}
