// SPDX-License-Identifier: GPL-3.0-or-later
//! The Zestaw view: preparing the service with two hands. Choosing, naming and deleting sets,
//! the items in order (move, remove with undo), adding songs, announcements, pictures and
//! pauses. Nothing here goes on screen by itself; showing is the Msza view's job (or "Pokaż
//! teraz", which says so).

mod add;
mod items;
mod picker;
mod review;
mod song_card;

use leptos::prelude::*;
use sursum_protocol::RemoteState;

use crate::Commands;
use crate::messages::{Msg, pl};

pub use song_card::SongCard;

/// The whole view.
#[component]
pub fn SetsView(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let searching = move || !query.with(|query| query.trim().is_empty());
    let card = RwSignal::new(None::<sursum_protocol::SongId>);
    let items = move || view! { <items::Items state commands card /> };
    let card_sheet =
        move || card.get().map(|song_id| view! { <SongCard song_id open=card state commands /> });
    view! {
        <div class="sets-view" data-testid="sets-view">
            <picker::SetHeader state commands />
            <review::SetReviewLine state commands />
            <crate::review::ReviewLine state />
            <add::AddBar query commands />
            <Show when=searching fallback=items>
                <add::Results query state commands card />
            </Show>
        </div>
        {card_sheet}
    }
}

/// The empty state of the view: no set yet.
fn no_set() -> impl IntoView {
    view! { <p class="hint">{pl(Msg::NoSetHint)}</p> }
}
