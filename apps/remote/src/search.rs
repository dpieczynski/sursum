// SPDX-License-Identifier: GPL-3.0-or-later
//! Song search: the query goes to the server (`GET /api/songs`), which does all matching.

use leptos::prelude::*;
use sursum_protocol::{Command, SongSummary};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::Commands;
use crate::messages::{Msg, pl};

/// Wait after the last keystroke before asking the server.
const DEBOUNCE_MS: u32 = 150;

/// Asks the server for songs matching `query`; an empty list on any error.
async fn fetch_songs(query: &str) -> Vec<SongSummary> {
    let url = format!("/api/songs?q={}", js_sys::encode_uri_component(query));
    let Some(window) = web_sys::window() else { return Vec::new() };
    let Ok(response) = JsFuture::from(window.fetch_with_str(&url)).await else { return Vec::new() };
    let Ok(response) = response.dyn_into::<web_sys::Response>() else { return Vec::new() };
    let Ok(text) = response.text() else { return Vec::new() };
    let text = JsFuture::from(text).await.ok().and_then(|text| text.as_string()).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// The search field; while it is not empty, results replace the set list.
#[component]
pub fn SearchBox(query: RwSignal<String>) -> impl IntoView {
    let on_input = move |event| query.set(event_target_value(&event));
    let clear = move |_| query.set(String::new());
    view! {
        <div class="search">
            <input
                class="search-input"
                type="text"
                enterkeyhint="search"
                autocomplete="off"
                placeholder=pl(Msg::SearchPlaceholder)
                prop:value=query
                on:input=on_input
            />
            <button class="search-clear" on:click=clear>
                "×"
            </button>
        </div>
    }
}

/// Results for the current query, fetched with a short debounce.
#[component]
pub fn SearchResults(query: RwSignal<String>, commands: Commands) -> impl IntoView {
    let results = RwSignal::new(Vec::<SongSummary>::new());
    let generation = StoredValue::new(0_u32);
    Effect::new(move |_| {
        let text = query.get();
        generation.update_value(|value| *value += 1);
        let this_request = generation.get_value();
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
            if generation.get_value() != this_request {
                return;
            }
            let songs = if text.trim().is_empty() { Vec::new() } else { fetch_songs(&text).await };
            // A newer query may have finished first; keep only the latest.
            if generation.get_value() == this_request {
                results.set(songs);
            }
        });
    });
    let rows = move || {
        let songs = results.get();
        if songs.is_empty() {
            return view! { <li class="search-empty">{pl(Msg::NoResults)}</li> }.into_any();
        }
        songs.into_iter().map(|song| result_row(song, query, commands)).collect_view().into_any()
    };
    view! { <ul class="set-list search-results">{rows}</ul> }
}

/// One result with its two actions; both clear the search afterwards.
fn result_row(song: SongSummary, query: RwSignal<String>, commands: Commands) -> impl IntoView {
    let (show_id, add_id) = (song.song_id.clone(), song.song_id.clone());
    let show = move |_| {
        commands.send(Command::ShowNow { song_id: show_id.clone() });
        query.set(String::new());
    };
    let add = move |_| {
        commands.send(Command::AddSong { song_id: add_id.clone() });
        query.set(String::new());
    };
    let number = song.number.unwrap_or_default();
    view! {
        <li class="set-item search-result">
            <span class="number">{number}</span>
            <span class="result-text">
                <span class="result-title">{song.title}</span>
                <span class="result-line">{song.first_line}</span>
            </span>
            <button class="result-action" on:click=add>
                {pl(Msg::Add)}
            </button>
            <button class="result-action show" data-testid="show-now" on:click=show>
                {pl(Msg::ShowNow)}
            </button>
        </li>
    }
}
