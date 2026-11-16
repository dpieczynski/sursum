// SPDX-License-Identifier: GPL-3.0-or-later
//! The items of the active set, always editable: move up and down, remove with a hold (and
//! undo). Tapping a song opens its card; nothing here goes on screen.

use leptos::prelude::*;
use sursum_protocol::{Command, ItemId, RemoteState, SetItemKind, SetItemView, SongId};

use crate::Commands;
use crate::messages::{Msg, pl, removed};
use crate::ui::{HoldButton, Toasts};

#[component]
pub fn Items(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    card: RwSignal<Option<SongId>>,
) -> impl IntoView {
    let live_item = Signal::derive(move || {
        state.with(|state| state.as_ref()?.program.live.as_ref().map(|live| live.deck.item_id.clone()))
    });
    let items = move || {
        state.with(|state| {
            state.as_ref().and_then(|state| state.active_set.as_ref().map(|set| set.items.clone()))
        })
    };
    let rows = move || match items() {
        None => super::no_set().into_any(),
        Some(items) if items.is_empty() => view! { <p class="hint">{pl(Msg::SetEmptyHint)}</p> }.into_any(),
        Some(items) => {
            let count = items.len() as u32;
            items
                .into_iter()
                .map(|item| row(item, count, live_item, commands, card))
                .collect_view()
                .into_any()
        }
    };
    let can_undo = move || state.with(|state| state.as_ref().is_some_and(|state| state.can_undo));
    view! {
        <ol class="edit-rows" data-testid="set-items">
            {rows}
        </ol>
        <Show when=can_undo>
            <button
                class="btn btn--sm btn--quiet undo-remove"
                data-testid="undo-remove"
                on:click=move |_| commands.send(Command::UndoRemove)
            >
                {pl(Msg::Undo)}
            </button>
        </Show>
    }
}

fn row(
    item: SetItemView,
    count: u32,
    live_item: Signal<Option<ItemId>>,
    commands: Commands,
    card: RwSignal<Option<SongId>>,
) -> impl IntoView {
    let number = item.number;
    let live_id = item.item_id.clone();
    let is_live = move || live_item.with(|live| live.as_ref() == Some(&live_id));
    let title = match item.kind {
        SetItemKind::MissingSong => pl(Msg::DeletedSong).to_owned(),
        _ if item.title.is_empty() => pl(Msg::Untitled).to_owned(),
        _ => item.title.clone(),
    };
    let kind = match item.kind {
        SetItemKind::Song | SetItemKind::MissingSong => item.song_number.clone().unwrap_or_default(),
        SetItemKind::Text => pl(Msg::KindText).to_owned(),
        SetItemKind::Image => pl(Msg::KindImage).to_owned(),
        SetItemKind::Pause => pl(Msg::KindPause).to_owned(),
    };
    let song_id = item.song_id.clone().filter(|_| item.kind == SetItemKind::Song);
    let open_card = move |_| {
        if let Some(song_id) = song_id.clone() {
            card.set(Some(song_id));
        }
    };
    let index = number - 1;
    let (up_id, down_id, remove_id) = (item.item_id.clone(), item.item_id.clone(), item.item_id.clone());
    let up = move |_| {
        commands.send(Command::MoveItem { item_id: up_id.clone(), to_index: index.saturating_sub(1) });
    };
    let down = move |_| commands.send(Command::MoveItem { item_id: down_id.clone(), to_index: index + 1 });
    let toasts = expect_context::<Toasts>();
    let toast_title = title.clone();
    let remove = move || {
        if commands.try_send(Command::RemoveItem { item_id: remove_id.clone() }) {
            toasts.show_with_undo(removed(&toast_title), Command::UndoRemove);
        }
    };
    view! {
        <li class="edit-row" class:live=is_live data-number=number>
            <span class="row-number">{number}</span>
            <button class="edit-title tappable" on:click=open_card>
                <span class="row-title">{title}</span>
                <span class="row-kind">{kind}</span>
            </button>
            <button class="btn btn--sm move-up" aria-label=pl(Msg::MoveUp) disabled=index == 0 on:click=up>
                "↑"
            </button>
            <button
                class="btn btn--sm move-down"
                aria-label=pl(Msg::MoveDown)
                disabled=number == count
                on:click=down
            >
                "↓"
            </button>
            <HoldButton label=pl(Msg::Remove) on_hold=remove testid="remove-item" />
        </li>
    }
}
