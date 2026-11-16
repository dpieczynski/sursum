// SPDX-License-Identifier: GPL-3.0-or-later
//! The active set: switching sets, the running order, and editing it during the service.
//! Editing never changes the screen; only tapping an item does (ADR 0004, ADR 0005).

use leptos::prelude::*;
use sursum_protocol::{Command, ItemId, RemoteState, SetId, SetItemView};

use crate::Commands;
use crate::hold::HoldButton;
use crate::messages::{Msg, pl};

/// Set selector, edit toggle, the item list and undo.
#[component]
pub fn SetPanel(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let editing = RwSignal::new(false);
    let announcement = RwSignal::new(false);
    let can_undo = move || state.with(|state| state.as_ref().is_some_and(|state| state.can_undo));
    let undo = move |_| commands.send(Command::UndoRemove);
    let toggle = move |_| editing.update(|on| *on = !*on);
    let edit_label = move || if editing.get() { pl(Msg::DoneEditing) } else { pl(Msg::EditSet) };
    let dialog =
        move || announcement.get().then(|| view! { <AnnouncementDialog open=announcement commands /> });
    view! {
        <div class="set-header">
            <SetSelector state commands />
            <button class="set-tool" on:click=toggle>
                {edit_label}
            </button>
            <button class="set-tool" on:click=move |_| announcement.set(true)>
                {pl(Msg::Announcement)}
            </button>
            <Show when=can_undo>
                <button class="set-tool undo" on:click=undo>
                    {pl(Msg::Undo)}
                </button>
            </Show>
        </div>
        {dialog}
        <SetList state commands editing />
    }
}

/// Picks the active set or creates a new one.
#[component]
fn SetSelector(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let options = move || {
        let (sets, active) = state.with(|state| match state {
            Some(state) => (state.sets.clone(), state.active_set.as_ref().map(|set| set.set_id.clone())),
            None => (Vec::new(), None),
        });
        sets.into_iter()
            .map(|set| {
                let selected = active.as_ref() == Some(&set.set_id);
                view! {
                    <option value=set.set_id.0.clone() selected=selected>
                        {set.name}
                    </option>
                }
            })
            .collect_view()
    };
    let on_change = move |event| {
        commands.send(Command::ActivateSet { set_id: SetId(event_target_value(&event)) });
    };
    let new_set = move |_| {
        let name = web_sys::window()
            .and_then(|window| window.prompt_with_message(pl(Msg::NewSetPrompt)).ok().flatten());
        if let Some(name) = name {
            commands.send(Command::NewSet { name });
        }
    };
    view! {
        <select class="set-select" on:change=on_change>
            {options}
        </select>
        <button class="set-tool" on:click=new_set>
            {pl(Msg::NewSet)}
        </button>
    }
}

#[component]
fn SetList(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    editing: RwSignal<bool>,
) -> impl IntoView {
    let live_item = Signal::derive(move || {
        state.with(|state| {
            state.as_ref().and_then(|s| s.program.live.as_ref().map(|live| live.deck.item_id.clone()))
        })
    });
    let rows = move || {
        let items = state.with(|state| {
            state
                .as_ref()
                .and_then(|s| s.active_set.as_ref().map(|set| set.items.clone()))
                .unwrap_or_default()
        });
        let count = items.len() as u32;
        items.into_iter().map(|item| set_row(item, count, live_item, editing, commands)).collect_view()
    };
    view! { <ul class="set-list">{rows}</ul> }
}

/// One entry: tapping shows its first slide; in edit mode it can be moved or removed.
fn set_row(
    item: SetItemView,
    count: u32,
    live_item: Signal<Option<ItemId>>,
    editing: RwSignal<bool>,
    commands: Commands,
) -> impl IntoView {
    let number = item.number;
    let item_id = item.item_id.clone();
    let is_live = move || live_item.with(|live| live.as_ref() == Some(&item_id));
    let title = if item.title.is_empty() { pl(Msg::Untitled).to_owned() } else { item.title.clone() };
    let show = move |_| {
        if !editing.get_untracked() {
            commands.send(Command::GoToItemNumber { number });
        }
    };
    let controls = move || editing.get().then(|| row_controls(item.item_id.clone(), number, count, commands));
    view! {
        <li class="set-item" class:live=is_live on:click=show>
            <span class="number">{number}</span>
            <span class="set-title">{title}</span>
            {controls}
        </li>
    }
}

/// Move up, move down and hold-to-remove buttons of one item.
fn row_controls(item_id: ItemId, number: u32, count: u32, commands: Commands) -> impl IntoView {
    let index = number - 1;
    let (up_id, down_id, remove_id) = (item_id.clone(), item_id.clone(), item_id);
    let up = move |_| {
        commands.send(Command::MoveItem { item_id: up_id.clone(), to_index: index.saturating_sub(1) });
    };
    let down = move |_| commands.send(Command::MoveItem { item_id: down_id.clone(), to_index: index + 1 });
    let remove = move || commands.send(Command::RemoveItem { item_id: remove_id.clone() });
    view! {
        <span class="row-controls">
            <button class="row-button" disabled=index == 0 on:click=up>
                "↑"
            </button>
            <button class="row-button" disabled=number == count on:click=down>
                "↓"
            </button>
            <HoldButton label=pl(Msg::Remove) on_hold=remove />
        </span>
    }
}

/// A small form for an announcement (text slides; an empty line starts a new slide).
#[component]
fn AnnouncementDialog(open: RwSignal<bool>, commands: Commands) -> impl IntoView {
    let title = RwSignal::new(pl(Msg::Announcement).to_owned());
    let text = RwSignal::new(String::new());
    let add = move |_| {
        commands.send(Command::AddText { title: title.get_untracked(), text: text.get_untracked() });
        open.set(false);
    };
    view! {
        <div class="dialog">
            <label>
                {pl(Msg::EditorTitle)}
                <input prop:value=title on:input=move |e| title.set(event_target_value(&e)) />
            </label>
            <label>
                {pl(Msg::AnnouncementText)}
                <textarea
                    rows="6"
                    prop:value=text
                    on:input=move |e| text.set(event_target_value(&e))
                ></textarea>
            </label>
            <div class="editor-buttons">
                <button type="button" on:click=move |_| open.set(false)>
                    {pl(Msg::Cancel)}
                </button>
                <button type="button" class="primary" on:click=add>
                    {pl(Msg::Add)}
                </button>
            </div>
        </div>
    }
}
