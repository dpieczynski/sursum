// SPDX-License-Identifier: GPL-3.0-or-later
//! "Szukaj" during Mass: find a song that is not in the set (or is, further down) and show it
//! at once, or add it to the end of the set. The controls stay usable beside or below the sheet
//! (see `ui::Sheet`), so black is one tap away while searching.

use leptos::prelude::*;
use sursum_protocol::{Command, SongSummary};

use crate::Commands;
use crate::messages::{Msg, pl};
use crate::search::Search;
use crate::ui::{Sheet, Toasts};

/// Wait after the last keystroke before asking the server.
const DEBOUNCE_MS: u32 = 150;

#[component]
pub fn SearchSheet(commands: Commands, open: RwSignal<bool>) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let numbers = RwSignal::new(false);
    let results = search_results(query);
    let toasts = expect_context::<Toasts>();
    let rows = move || {
        let search = results.get();
        if let Some(notice) = search.notice() {
            return view! { <li class="search-empty">{pl(notice)}</li> }.into_any();
        }
        let Search::Found(songs) = search else { return ().into_any() };
        if songs.is_empty() {
            return view! { <li class="search-empty">{pl(Msg::NoResults)}</li> }.into_any();
        }
        songs
            .into_iter()
            .map(|song| result_row(song, commands, open, query, toasts))
            .collect_view()
            .into_any()
    };
    let input_mode = move || if numbers.get() { "numeric" } else { "search" };
    view! {
        <Sheet title=pl(Msg::SearchSong).to_owned() on_close=move || open.set(false) testid="search-sheet">
            <div class="search">
                <input
                    class="search-input"
                    type="search"
                    inputmode=input_mode
                    enterkeyhint="search"
                    autocomplete="off"
                    autocapitalize="off"
                    spellcheck="false"
                    autofocus=true
                    placeholder=pl(Msg::SearchPlaceholder)
                    prop:value=query
                    on:input=move |event| query.set(event_target_value(&event))
                />
                <button
                    class="btn btn--sm btn--quiet"
                    aria-pressed=move || numbers.get().to_string()
                    on:click=move |_| numbers.update(|on| *on = !*on)
                >
                    {pl(Msg::Numbers)}
                </button>
            </div>
            <ul class="search-results">{rows}</ul>
        </Sheet>
    }
}

/// Results for `query`, fetched with a short debounce; only the latest answer counts. While a
/// newer answer is on its way the previous results stay, so the list does not flicker.
pub fn search_results(query: RwSignal<String>) -> RwSignal<Search> {
    let results = RwSignal::new(Search::Idle);
    let generation = StoredValue::new(0_u32);
    Effect::new(move |_| {
        let text = query.get();
        generation.update_value(|value| *value += 1);
        let this_request = generation.get_value();
        if text.trim().is_empty() {
            results.set(Search::Idle);
            return;
        }
        if !matches!(results.get_untracked(), Search::Found(_)) {
            results.set(Search::Waiting);
        }
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
            if generation.get_value() != this_request {
                return;
            }
            let answer = crate::search::fetch_songs(&text).await;
            if generation.get_value() == this_request {
                results.set(answer.map_or(Search::Failed, Search::Found));
            }
        });
    });
    results
}

fn result_row(
    song: SongSummary,
    commands: Commands,
    open: RwSignal<bool>,
    query: RwSignal<String>,
    toasts: Toasts,
) -> impl IntoView {
    let (show_id, add_id, title) = (song.song_id.clone(), song.song_id.clone(), song.title.clone());
    let show = move |_| {
        if commands.try_send(Command::ShowNow { song_id: show_id.clone() }) {
            open.set(false);
        }
    };
    let add = move |_| {
        if commands.try_send(Command::AddSong { song_id: add_id.clone() }) {
            toasts.show(format!("{}: {title}", pl(Msg::Added)));
            query.set(String::new());
        }
    };
    view! {
        <li class="search-result">
            <span class="row-number">{song.number.unwrap_or_default()}</span>
            <span class="result-text">
                <span class="result-title">{song.title}</span>
                <span class="result-line">{song.first_line}</span>
            </span>
            <button class="btn btn--sm btn--quiet" data-testid="add-to-set" on:click=add>
                {pl(Msg::AddToSet)}
            </button>
            <button class="btn btn--primary result-show" data-testid="show-now" on:click=show>
                {pl(Msg::ShowNowLong)}
            </button>
        </li>
    }
}
