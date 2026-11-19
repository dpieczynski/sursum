// SPDX-License-Identifier: GPL-3.0-or-later
//! The set's name and the ways to change it: pick another set (by date, ADR 0025), create one,
//! copy one ("Nowy zestaw z tego"), rename it or change its date, delete it (hold, then undo
//! from the toast).

use leptos::prelude::*;
use sursum_protocol::{Command, RemoteState, SetId, SetSummary};

use crate::Commands;
use crate::messages::{Msg, pl, removed, set_date_label};
use crate::ui::{HoldButton, Sheet, Toasts};

/// Which sheet is open.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Open {
    Picker,
    New,
    Rename,
    /// A new set copied from this one.
    Copy(SetId),
}

#[component]
pub fn SetHeader(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let open = RwSignal::new(None::<Open>);
    let active = move || {
        state.with(|state| {
            state.as_ref().and_then(|state| {
                state.active_set.as_ref().map(|set| (set.set_id.clone(), set.name.clone(), set.date.clone()))
            })
        })
    };
    let name = move || active().map(|(_, name, _)| name);
    let date = move || active().and_then(|(_, _, date)| date).map(|date| set_date_label(&date));
    let has_set = move || active().is_some();
    let copy_active = move |_| {
        if let Some((set_id, _, _)) = active() {
            open.set(Some(Open::Copy(set_id)));
        }
    };
    let sheet = move || match open.get() {
        Some(Open::Picker) => view! { <Picker state commands open /> }.into_any(),
        Some(mode) => view! { <NameSheet state commands open mode /> }.into_any(),
        None => ().into_any(),
    };
    view! {
        <header class="sets-header">
            <h2 class="sets-name" data-testid="set-name">
                {move || name().unwrap_or_else(|| pl(Msg::NoSet).to_owned())}
                <span class="sets-date" data-testid="set-date">
                    {date}
                </span>
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
                    <button class="btn btn--sm btn--quiet" data-testid="copy-set" on:click=copy_active>
                        {pl(Msg::CopySet)}
                    </button>
                </Show>
            </div>
        </header>
        {sheet}
    }
}

/// All sets of this church, by date (sets without one first); the chosen one becomes active.
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
    let (pick_id, delete_id, copy_id) = (set.set_id.clone(), set.set_id.clone(), set.set_id.clone());
    let pick = move |_| {
        commands.send(Command::ActivateSet { set_id: pick_id.clone() });
        open.set(None);
    };
    let copy = move |_| open.set(Some(Open::Copy(copy_id.clone())));
    let toasts = expect_context::<Toasts>();
    let deleted_name = set.name.clone();
    let delete = move || {
        if commands.try_send(Command::DeleteSet { set_id: delete_id.clone() }) {
            toasts.show_with_undo(removed(&deleted_name), Command::UndoDeleteSet);
        }
    };
    let date = set.date.as_deref().map(set_date_label);
    view! {
        <li class="picker-row" class:chosen=chosen>
            <button class="picker-pick tappable" data-testid="pick-set" on:click=pick>
                <span class="picker-name">{set.name}</span>
                <span class="picker-count">
                    {date.map(|date| format!("{date} · "))} {format!("{} {}", set.items, pl(Msg::Items))}
                </span>
                {chosen.then(|| view! { <span class="row-badge muted">{pl(Msg::Chosen)}</span> })}
            </button>
            <button class="btn btn--sm btn--quiet" data-testid="copy-set-row" on:click=copy>
                {pl(Msg::CopySetShort)}
            </button>
            <HoldButton label=pl(Msg::Delete) on_hold=delete testid="delete-set" />
        </li>
    }
}

/// What the name sheet starts with: the set it is about (none for a new one), its name and date.
fn starting_point(state: RwSignal<Option<RemoteState>>, mode: &Open) -> (Option<SetId>, String, String) {
    state.with_untracked(|state| {
        let state = state.as_ref();
        match mode {
            Open::Rename => state
                .and_then(|state| state.active_set.as_ref())
                .map(|set| (Some(set.set_id.clone()), set.name.clone(), set.date.clone().unwrap_or_default()))
                .unwrap_or_default(),
            // A copy keeps the name (to edit) and asks for a new date.
            Open::Copy(set_id) => state
                .and_then(|state| state.sets.iter().find(|set| &set.set_id == set_id))
                .map(|set| (Some(set.set_id.clone()), set.name.clone(), String::new()))
                .unwrap_or_default(),
            Open::New | Open::Picker => (None, String::new(), String::new()),
        }
    })
}

/// A new set's name and date, a new name and date for the active one, or a copy's.
#[component]
fn NameSheet(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    open: RwSignal<Option<Open>>,
    mode: Open,
) -> impl IntoView {
    let (set_id, start_name, start_date) = starting_point(state, &mode);
    let name = RwSignal::new(start_name.clone());
    // The date field gives `YYYY-MM-DD`; a time from elsewhere is kept as it is.
    let date = RwSignal::new(start_date.clone());
    let submit_mode = mode.clone();
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let (typed, chosen) = (name.get_untracked(), date.get_untracked());
        let date = (!chosen.trim().is_empty()).then(|| chosen.clone());
        match (&submit_mode, &set_id) {
            (Open::Rename, Some(set_id)) => {
                if typed != start_name {
                    commands.send(Command::RenameSet { set_id: set_id.clone(), name: typed });
                }
                if chosen != start_date {
                    commands.send(Command::DateSet { set_id: set_id.clone(), date });
                }
            }
            (Open::Copy(_), Some(set_id)) => {
                commands.send(Command::CopySet { set_id: set_id.clone(), name: typed, date, church: None });
            }
            _ => commands.send(Command::NewSet { name: typed, date }),
        }
        open.set(None);
    };
    let day = move || date.get().chars().take(10).collect::<String>();
    let (title, action) = match mode {
        Open::Rename => (pl(Msg::Rename), pl(Msg::Save)),
        Open::Copy(_) => (pl(Msg::CopySet), pl(Msg::Create)),
        Open::New | Open::Picker => (pl(Msg::NewSet), pl(Msg::Create)),
    };
    view! {
        <Sheet title=title.to_owned() on_close=move || open.set(None) testid="set-name-sheet">
            <form class="sheet-form" on:submit=submit>
                <label class="field">
                    {pl(Msg::SetName)}
                    <input
                        autofocus=true
                        data-testid="set-name-input"
                        prop:value=name
                        on:input=move |event| name.set(event_target_value(&event))
                    />
                </label>
                <label class="field">
                    {pl(Msg::SetDate)}
                    <input
                        type="date"
                        data-testid="set-date-input"
                        prop:value=day
                        on:input=move |event| date.set(event_target_value(&event))
                    />
                </label>
                <div class="sheet-actions">
                    <button type="submit" class="btn btn--primary">
                        {action}
                    </button>
                </div>
            </form>
        </Sheet>
    }
}
