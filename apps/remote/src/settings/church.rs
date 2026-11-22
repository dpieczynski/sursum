// SPDX-License-Identifier: GPL-3.0-or-later
//! Kościół: the church this device serves (ADR 0025). Its sets are this church's sets; songs are
//! shared by the parish. The name can be corrected here; if it was typed differently on another
//! device, this device can join the church it learned from a library file.

use leptos::prelude::*;
use sursum_protocol::{ChurchView, Command, RemoteState};

use super::Section;
use crate::Commands;
use crate::messages::{Msg, pl};

#[component]
pub fn ChurchSection(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    // Memos: the field is refilled only when the church really changes on the server, not on
    // every snapshot (which would wipe a half-typed name).
    let church_memo =
        Memo::new(move |_| state.with(|state| state.as_ref().and_then(|state| state.church.clone())));
    let church = move || church_memo.get();
    let others_memo = Memo::new(move |_| {
        state.with(|state| {
            let Some(state) = state.as_ref() else { return Vec::new() };
            let own = state.church.as_ref().map(|church| &church.church_id);
            state.churches.iter().filter(|church| Some(&church.church_id) != own).cloned().collect::<Vec<_>>()
        })
    });
    let others = move || others_memo.get();
    let name = RwSignal::new(String::new());
    // Show the current name in the field whenever it changes on the server.
    Effect::new(move |_| name.set(church().map(|church| church.name).unwrap_or_default()));
    let save = move |_| commands.send(Command::NameChurch { name: name.get_untracked() });
    let none_yet = move || church().is_none().then(|| view! { <p class="hint">{pl(Msg::ChurchNoneYet)}</p> });
    let other_list = move || {
        let others = others();
        (!others.is_empty()).then(|| {
            let buttons = others.into_iter().map(|church| other_church(church, commands)).collect_view();
            view! {
                <p class="setting-label">{pl(Msg::ChurchOther)}</p>
                <p class="hint">{pl(Msg::ChurchOtherHint)}</p>
                <div class="setting-row">{buttons}</div>
            }
        })
    };
    view! {
        <Section title=Msg::ChurchSection>
            {none_yet} <div class="setting-row">
                <label class="field">
                    {pl(Msg::ChurchName)}
                    <input
                        data-testid="church-name"
                        placeholder=pl(Msg::ChurchNameExample)
                        prop:value=name
                        on:input=move |event| name.set(event_target_value(&event))
                    />
                </label>
                <button class="btn btn--sm" data-testid="church-save" on:click=save>
                    {pl(Msg::ChurchSave)}
                </button>
            </div> {other_list}
        </Section>
    }
}

fn other_church(church: ChurchView, commands: Commands) -> impl IntoView {
    let church_id = church.church_id;
    let join = move |_| commands.send(Command::UseChurch { church_id: church_id.clone() });
    view! {
        <button class="btn btn--sm" data-testid="church-use" on:click=join>
            {church.name}
        </button>
    }
}
