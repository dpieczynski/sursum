// SPDX-License-Identifier: GPL-3.0-or-later
//! The desktop program prepares sets for every church of the parish (ADR 0025): this chooses
//! whose sets the Zestaw view lists, and whose new sets are. A device with a screen serves one
//! church (named at setup) and shows nothing here.

use leptos::prelude::*;
use sursum_protocol::{ChurchId, ChurchView, Command, Installation, RemoteState};

use crate::Commands;
use crate::messages::{Msg, pl};

#[component]
pub fn ChurchChoice(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let library = expect_context::<Installation>() == Installation::Library;
    let churches =
        move || state.with(|state| state.as_ref().map(|state| state.churches.clone()).unwrap_or_default());
    let chosen = move || {
        state.with(|state| {
            state
                .as_ref()
                .and_then(|state| state.church.as_ref())
                .map(|church| church.church_id.as_str().to_owned())
                .unwrap_or_default()
        })
    };
    let change = move |event: leptos::ev::Event| {
        let church_id = event_target_value(&event);
        if !church_id.is_empty() {
            commands.send(Command::UseChurch { church_id: ChurchId::from(church_id.as_str()) });
        }
    };
    let options = move || church_options(churches(), chosen().is_empty());
    let has_churches = move || !churches().is_empty();
    let no_churches = || view! { <p class="hint">{pl(Msg::NoChurchesHere)}</p> };
    view! {
        <Show when=move || library>
            <Show when=has_churches fallback=no_churches>
                <label class="field church-choice">
                    {pl(Msg::SetsOfChurch)}
                    <select data-testid="church-choice" prop:value=chosen on:change=change>
                        {options}
                    </select>
                </label>
            </Show>
        </Show>
    }
}

/// One option per church; before one is chosen, a first option says so.
fn church_options(churches: Vec<ChurchView>, nothing_chosen: bool) -> impl IntoView {
    let placeholder = nothing_chosen.then(|| view! { <option value="">{pl(Msg::ChooseChurch)}</option> });
    let churches = churches
        .into_iter()
        .map(|church| {
            let value = church.church_id.as_str().to_owned();
            view! { <option value=value>{church.name}</option> }
        })
        .collect_view();
    view! {
        {placeholder}
        {churches}
    }
}
