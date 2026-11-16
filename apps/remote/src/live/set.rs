// SPDX-License-Identifier: GPL-3.0-or-later
//! The running order during Mass: a tap shows the item (and lifts black). The live item is
//! marked and scrolled to the second place, the next item is outlined. No editing here; that is
//! the Zestaw view.

use leptos::prelude::*;
use sursum_protocol::{Command, ItemId, NextStep, RemoteState, SetItemKind, SetItemView};

use crate::Commands;
use crate::messages::{Msg, pl};
use crate::shell::{CurrentView, View};

#[component]
pub fn LiveSet(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let container = NodeRef::<leptos::html::Ol>::new();
    let live_item = Signal::derive(move || {
        state.with(|state| state.as_ref()?.program.live.as_ref().map(|live| live.deck.item_id.clone()))
    });
    let next_item = Signal::derive(move || {
        state.with(|state| {
            let state = state.as_ref()?;
            (state.next_step == NextStep::Item)
                .then(|| state.next.as_ref().map(|next| next.deck.item_id.clone()))?
        })
    });
    let header = move || {
        state.with(|state| {
            state
                .as_ref()
                .and_then(|state| state.active_set.as_ref())
                .map(|set| format!("{} · {} {}", set.name, set.items.len(), pl(Msg::Items)))
        })
    };
    let items = move || {
        state.with(|state| {
            state.as_ref().and_then(|state| state.active_set.as_ref().map(|set| set.items.clone()))
        })
    };
    let rows = move || match items() {
        None => empty(Msg::NoSet, Msg::ChooseSet).into_any(),
        Some(items) if items.is_empty() => empty(Msg::SetEmpty, Msg::AddSongs).into_any(),
        Some(items) => {
            items.into_iter().map(|item| row(item, live_item, next_item, commands)).collect_view().into_any()
        }
    };
    Effect::new(move |_| {
        live_item.track();
        crate::scroll::reveal_after_render(container, ".set-row.live", crate::scroll::Place::Second);
    });
    view! {
        <div class="live-set">
            <h2 class="live-set-header">{header}</h2>
            <ol class="set-rows" node_ref=container data-testid="live-set">
                {rows}
            </ol>
        </div>
    }
}

fn row(
    item: SetItemView,
    live_item: Signal<Option<ItemId>>,
    next_item: Signal<Option<ItemId>>,
    commands: Commands,
) -> impl IntoView {
    let number = item.number;
    let (live_id, next_id) = (item.item_id.clone(), item.item_id.clone());
    let is_live = Signal::derive(move || live_item.with(|live| live.as_ref() == Some(&live_id)));
    let is_next = Signal::derive(move || next_item.with(|next| next.as_ref() == Some(&next_id)));
    let missing = item.kind == SetItemKind::MissingSong;
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
    let connection = expect_context::<crate::Connection>();
    let show = move |_| commands.send(Command::GoToItemNumber { number });
    let badge = move || {
        if is_live.get() {
            Some(view! { <span class="row-badge live">{pl(Msg::LiveBadge)}</span> })
        } else {
            is_next.get().then(|| view! { <span class="row-badge next">{pl(Msg::NextBadge)}</span> })
        }
    };
    view! {
        <li>
            <button
                class="set-row set-item tappable"
                class:live=is_live
                class:next=is_next
                prop:disabled=move || missing || !connection.up()
                data-number=number
                on:click=show
            >
                <span class="row-number">{number}</span>
                <span class="row-title">{title}</span>
                {badge}
                <span class="row-kind">{kind}</span>
            </button>
        </li>
    }
}

/// An empty state with a way out: to the Zestaw view.
fn empty(message: Msg, action: Msg) -> impl IntoView {
    let current = expect_context::<CurrentView>();
    view! {
        <li class="set-empty">
            <p>{pl(message)}</p>
            <button class="btn" on:click=move |_| current.show(View::Set)>
                {pl(action)}
            </button>
        </li>
    }
}
