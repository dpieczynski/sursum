// SPDX-License-Identifier: GPL-3.0-or-later
//! The Pieśni view of the desktop program's library (ADR 0020): there is no screen and no
//! service at home, only songs. Search, open a song in the editor, or write a new one; the
//! finished library goes to the church as a file (Ustawienia → Eksport bazy).

use leptos::prelude::*;
use sursum_protocol::SongSummary;

use crate::editor::EditorTarget;
use crate::live::search_results;
use crate::messages::{Msg, new_song_named, pl};

#[component]
pub fn SongsView() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let results = search_results(query);
    let editing = expect_context::<RwSignal<Option<EditorTarget>>>();
    let new_song = move |_| editing.set(Some(EditorTarget::New { title: String::new() }));
    let rows = move || {
        let typed = query.get().trim().to_owned();
        if typed.is_empty() {
            return view! { <p class="hint">{pl(Msg::LibraryHint)}</p> }.into_any();
        }
        let songs = results.get();
        if songs.is_empty() {
            return no_results(typed, editing).into_any();
        }
        song_list(songs, editing).into_any()
    };
    view! {
        <div class="songs-view" data-testid="songs-view">
            <div class="add-bar">
                <div class="search">
                    <input
                        class="search-input"
                        type="search"
                        enterkeyhint="search"
                        autocomplete="off"
                        autocapitalize="off"
                        spellcheck="false"
                        data-testid="library-search"
                        placeholder=pl(Msg::SearchPlaceholder)
                        prop:value=query
                        on:input=move |event| query.set(event_target_value(&event))
                    />
                </div>
                <div class="add-buttons">
                    <button class="btn btn--primary new-song" data-testid="new-song" on:click=new_song>
                        {pl(Msg::NewSong)}
                    </button>
                </div>
            </div>
            {rows}
        </div>
    }
}

/// The songs found; tapping one opens the editor.
fn song_list(songs: Vec<SongSummary>, editing: RwSignal<Option<EditorTarget>>) -> impl IntoView {
    let rows = songs.into_iter().map(|song| song_row(song, editing)).collect_view();
    view! {
        <ul class="search-results" data-testid="library-results">
            {rows}
        </ul>
    }
}

/// Nothing found: offer a new song with the typed text as its title.
fn no_results(typed: String, editing: RwSignal<Option<EditorTarget>>) -> impl IntoView {
    let label = new_song_named(&typed);
    let create = move |_| editing.set(Some(EditorTarget::New { title: typed.clone() }));
    view! {
        <div class="search-empty">
            <p>{pl(Msg::NoResults)}</p>
            <button class="btn" data-testid="new-song-from-search" on:click=create>
                {label}
            </button>
        </div>
    }
}

/// One song: tapping it opens the editor.
fn song_row(song: SongSummary, editing: RwSignal<Option<EditorTarget>>) -> impl IntoView {
    let song_id = song.song_id.clone();
    let open = move |_| editing.set(Some(EditorTarget::Existing(song_id.clone())));
    view! {
        <li class="search-result">
            <span class="row-number">{song.number.unwrap_or_default()}</span>
            <button class="result-text tappable" data-testid="library-song" on:click=open>
                <span class="result-title">{song.title}</span>
                <span class="result-line">{song.first_line}</span>
            </button>
        </li>
    }
}
