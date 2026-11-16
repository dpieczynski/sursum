// SPDX-License-Identifier: GPL-3.0-or-later
//! The set's name and the ways to change it: pick another set (newest first), create one,
//! rename it, delete it (hold).

use leptos::prelude::*;
use sursum_protocol::{Command, RemoteState, SetId, SetSummary};

use crate::Commands;
use crate::messages::{Msg, pl};
use crate::ui::{HoldButton, Sheet};

/// Which sheet is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Picker,
    New,
    Rename,
}

#[component]
pub fn SetHeader(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let open = RwSignal::new(None::<Open>);
    let name = move || {
        state.with(|state| {
            state.as_ref().and_then(|state| state.active_set.as_ref().map(|set| set.name.clone()))
        })
    };
    let has_set = move || name().is_some();
    let sheet = move || match open.get() {
        Some(Open::Picker) => view! { <Picker state commands open /> }.into_any(),
        Some(Open::New) => view! { <NameSheet state commands open rename=false /> }.into_any(),
        Some(Open::Rename) => view! { <NameSheet state commands open rename=true /> }.into_any(),
        None => ().into_any(),
    };
    view! {
        <header class="sets-header">
            <h2 class="sets-name" data-testid="set-name">
                {move || name().unwrap_or_else(|| pl(Msg::NoSet).to_owned())}
            </h2>
            <div class="sets-header-tools">
                <button
                    class="btn btn--sm"
                    data-testid="change-set"
                    on:click=move |_| open.set(Some(Open::Picker))
                >
                    {pl(Msg::ChangeSet)}
                </button>
                <Show when=has_set>
                    <button
                        class="btn btn--sm btn--quiet"
                        data-testid="rename-set"
                        on:click=move |_| open.set(Some(Open::Rename))
                    >
                        {pl(Msg::Rename)}
                    </button>
                </Show>
            </div>
        </header>
        {sheet}
    }
}

/// All sets, newest first; the chosen one becomes active.
#[component]
fn Picker(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    open: RwSignal<Option<Open>>,
) -> impl IntoView {
    let close = move || open.set(None);
    let live = move || state.with(|state| state.as_ref().is_some_and(|state| state.program.live.is_some()));
    let rows = move || {
        let (sets, active) = state.with(|state| match state {
            Some(state) => (state.sets.clone(), state.active_set.as_ref().map(|set| set.set_id.clone())),
            None => (Vec::new(), None),
        });
        sets.into_iter().map(|set| set_row(set, active.clone(), commands, open)).collect_view()
    };
    view! {
        <Sheet title=pl(Msg::ChangeSet).to_owned() on_close=close testid="set-picker">
            <Show when=live>
                <p class="hint">{pl(Msg::SwitchSetHint)}</p>
            </Show>
            <ul class="picker-list">{rows}</ul>
            <div class="sheet-actions">
                <button
                    class="btn btn--primary"
                    data-testid="new-set"
                    on:click=move |_| open.set(Some(Open::New))
                >
                    {pl(Msg::NewSet)}
                </button>
            </div>
        </Sheet>
    }
}

fn set_row(
    set: SetSummary,
    active: Option<SetId>,
    commands: Commands,
    open: RwSignal<Option<Open>>,
) -> impl IntoView {
    let chosen = active.as_ref() == Some(&set.set_id);
    let (pick_id, delete_id) = (set.set_id.clone(), set.set_id.clone());
    let pick = move |_| {
        commands.send(Command::ActivateSet { set_id: pick_id.clone() });
        open.set(None);
    };
    let delete = move || commands.send(Command::DeleteSet { set_id: delete_id.clone() });
    view! {
        <li class="picker-row" class:chosen=chosen>
            <button class="picker-pick tappable" data-testid="pick-set" on:click=pick>
                <span class="picker-name">{set.name}</span>
                <span class="picker-count">{format!("{} {}", set.items, pl(Msg::Items))}</span>
                {chosen.then(|| view! { <span class="row-badge live">{pl(Msg::Chosen)}</span> })}
            </button>
            <HoldButton label=pl(Msg::Delete) on_hold=delete testid="delete-set" />
        </li>
    }
}

/// The name of a new set, or a new name for the active one.
#[component]
fn NameSheet(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    open: RwSignal<Option<Open>>,
    rename: bool,
) -> impl IntoView {
    let current = state.with_untracked(|state| {
        state
            .as_ref()
            .and_then(|state| state.active_set.as_ref().map(|set| (set.set_id.clone(), set.name.clone())))
    });
    let name = RwSignal::new(if rename {
        current.as_ref().map(|(_, name)| name.clone()).unwrap_or_default()
    } else {
        String::new()
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let typed = name.get_untracked();
        match (&current, rename) {
            (Some((set_id, _)), true) => {
                commands.send(Command::RenameSet { set_id: set_id.clone(), name: typed });
            }
            _ => commands.send(Command::NewSet { name: typed }),
        }
        open.set(None);
    };
    let title = if rename { pl(Msg::Rename) } else { pl(Msg::NewSet) };
    view! {
        <Sheet title=title.to_owned() on_close=move || open.set(None) testid="set-name-sheet">
            <form class="sheet-form" on:submit=submit>
                <label class="field">
                    {pl(Msg::SetName)}
                    <input
                        autofocus=true
                        prop:value=name
                        on:input=move |event| name.set(event_target_value(&event))
                    />
                </label>
                <div class="sheet-actions">
                    <button type="submit" class="btn btn--primary">
                        {if rename { pl(Msg::Save) } else { pl(Msg::Create) }}
                    </button>
                </div>
            </form>
        </Sheet>
    }
}
