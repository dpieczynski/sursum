// SPDX-License-Identifier: GPL-3.0-or-later
//! Adding to the set: a song from the library (the search stays open, so several can be added
//! in a row), an announcement, a picture, a pause, or a new song typed in the editor.

use leptos::prelude::*;
use sursum_protocol::{Command, RemoteState, SongId, SongSummary};

use crate::Commands;
use crate::editor::EditorTarget;
use crate::live::search_results;
use crate::messages::{Msg, pl};
use crate::ui::{Sheet, Toasts};

/// The search field and the buttons for other kinds of items.
#[component]
pub fn AddBar(query: RwSignal<String>, commands: Commands) -> impl IntoView {
    let announcement = RwSignal::new(false);
    let editing = expect_context::<RwSignal<Option<EditorTarget>>>();
    let add_pause = move |_| commands.send(Command::AddBlank { title: pl(Msg::Pause).to_owned() });
    let new_song = move |_| editing.set(Some(EditorTarget::New { title: String::new() }));
    let dialog =
        move || announcement.get().then(|| view! { <AnnouncementSheet open=announcement commands /> });
    view! {
        <div class="add-bar">
            <div class="search">
                <input
                    class="search-input"
                    type="search"
                    enterkeyhint="search"
                    autocomplete="off"
                    placeholder=pl(Msg::SearchPlaceholder)
                    prop:value=query
                    on:input=move |event| query.set(event_target_value(&event))
                />
                <Show when=move || !query.with(String::is_empty)>
                    <button
                        class="btn btn--sm btn--quiet"
                        data-testid="close-search"
                        on:click=move |_| query.set(String::new())
                    >
                        {pl(Msg::CloseSearch)}
                    </button>
                </Show>
            </div>
            <div class="add-buttons">
                <button
                    class="btn btn--sm"
                    data-testid="add-announcement"
                    on:click=move |_| announcement.set(true)
                >
                    {pl(Msg::Announcement)}
                </button>
                <crate::picture::AddPictureButton commands />
                <button class="btn btn--sm" data-testid="add-pause" on:click=add_pause>
                    {pl(Msg::Pause)}
                </button>
                <button class="btn btn--sm new-song" data-testid="new-song" on:click=new_song>
                    {pl(Msg::NewSong)}
                </button>
            </div>
        </div>
        {dialog}
    }
}

/// Search results with "+ Dodaj" (the search stays open) and a mark for songs already in the set.
#[component]
pub fn Results(
    query: RwSignal<String>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    card: RwSignal<Option<SongId>>,
) -> impl IntoView {
    let results = search_results(query);
    let in_set = Memo::new(move |_| {
        state.with(|state| {
            state
                .as_ref()
                .and_then(|state| state.active_set.as_ref())
                .map(|set| set.items.iter().filter_map(|item| item.song_id.clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        })
    });
    let editing = expect_context::<RwSignal<Option<EditorTarget>>>();
    let rows = move || {
        let songs = results.get();
        if songs.is_empty() {
            let title = query.get().trim().to_owned();
            let create = move |_| editing.set(Some(EditorTarget::New { title: title.clone() }));
            let label = crate::messages::new_song_named(query.get().trim());
            return view! {
                <li class="search-empty">
                    <p>{pl(Msg::NoResults)}</p>
                    <button class="btn" data-testid="new-song-from-search" on:click=create>
                        {label}
                    </button>
                </li>
            }
            .into_any();
        }
        songs.into_iter().map(|song| result_row(song, in_set, commands, card)).collect_view().into_any()
    };
    view! {
        <ul class="search-results" data-testid="set-search-results">
            {rows}
        </ul>
    }
}

fn result_row(
    song: SongSummary,
    in_set: Memo<Vec<SongId>>,
    commands: Commands,
    card: RwSignal<Option<SongId>>,
) -> impl IntoView {
    let toasts = expect_context::<Toasts>();
    let (mark_id, add_id, card_id, title) =
        (song.song_id.clone(), song.song_id.clone(), song.song_id.clone(), song.title.clone());
    let already = move || in_set.with(|ids| ids.contains(&mark_id));
    let add = move |_| {
        if commands.try_send(Command::AddSong { song_id: add_id.clone() }) {
            toasts.show(format!("{}: {title}", pl(Msg::Added)));
        }
    };
    view! {
        <li class="search-result">
            <span class="row-number">{song.number.unwrap_or_default()}</span>
            <button class="result-text tappable" on:click=move |_| card.set(Some(card_id.clone()))>
                <span class="result-title">{song.title}</span>
                <span class="result-line">{song.first_line}</span>
            </button>
            <Show when=already>
                <span class="row-badge next">{pl(Msg::InSet)}</span>
            </Show>
            <button class="btn btn--primary result-action" data-testid="add-song" on:click=add>
                {pl(Msg::AddShort)}
            </button>
        </li>
    }
}

/// An announcement: text slides, an empty line starts a new one.
#[component]
fn AnnouncementSheet(open: RwSignal<bool>, commands: Commands) -> impl IntoView {
    let title = RwSignal::new(pl(Msg::Announcement).to_owned());
    let text = RwSignal::new(String::new());
    let add = move |_| {
        commands.send(Command::AddText { title: title.get_untracked(), text: text.get_untracked() });
        open.set(false);
    };
    view! {
        <Sheet title=pl(Msg::Announcement).to_owned() on_close=move || open.set(false) testid="announcement">
            <label class="field">
                {pl(Msg::EditorTitle)}
                <input prop:value=title on:input=move |e| title.set(event_target_value(&e)) />
            </label>
            <label class="field">
                {pl(Msg::AnnouncementText)}
                <textarea
                    rows="6"
                    prop:value=text
                    on:input=move |e| text.set(event_target_value(&e))
                ></textarea>
            </label>
            <div class="sheet-actions">
                <button type="button" class="btn btn--primary" on:click=add>
                    {pl(Msg::Add)}
                </button>
            </div>
        </Sheet>
    }
}
