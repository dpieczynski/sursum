// SPDX-License-Identifier: GPL-3.0-or-later
//! A song's card: its slides as text (compiled by the server, as for the editor's preview) and
//! what can be done with it: add to the set, edit, or show now (which goes on screen at once).

use leptos::prelude::*;
use sursum_protocol::{Command, DraftPreview, RemoteState, SlideBody, SongDraft, SongId};

use crate::Commands;
use crate::api::{get_json, post_json};
use crate::editor::EditorTarget;
use crate::live::info::slide_name;
use crate::messages::{Msg, pl};
use crate::ui::{Sheet, Toasts};

#[component]
pub fn SongCard(
    song_id: SongId,
    open: RwSignal<Option<SongId>>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
) -> impl IntoView {
    let draft = RwSignal::new(None::<SongDraft>);
    let preview = RwSignal::new(None::<DraftPreview>);
    let url = format!("/api/songs/{}/draft", js_sys::encode_uri_component(song_id.as_str()));
    leptos::task::spawn_local(async move {
        let Some(loaded) = get_json::<SongDraft>(&url).await else { return };
        preview.set(post_json::<_, DraftPreview>("/api/songs/preview", &loaded).await);
        draft.set(Some(loaded));
    });
    let close = move || open.set(None);
    let title =
        move || draft.with(|draft| draft.as_ref().map(|draft| draft.title.clone()).unwrap_or_default());
    let number = move || draft.with(|draft| draft.as_ref().and_then(|draft| draft.number.clone()));
    let slides = move || {
        preview.with(|preview| {
            preview.as_ref().map(|preview| {
                preview
                    .deck
                    .slides
                    .iter()
                    .map(|slide| {
                        let name = slide_name(slide).unwrap_or_default();
                        let lines = match &slide.body {
                            SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => lines.join("\n"),
                            SlideBody::Image { .. } => String::new(),
                        };
                        view! {
                            <div class="card-slide">
                                <span class="slide-name">{name}</span>
                                <p class="card-lines">{lines}</p>
                            </div>
                        }
                    })
                    .collect_view()
            })
        })
    };
    let toasts = expect_context::<Toasts>();
    let editing = expect_context::<RwSignal<Option<EditorTarget>>>();
    // Opened from the set (or for a song already in it), adding is rarely what is wanted: it
    // stays possible, but is not the main button.
    let in_set = {
        let song_id = song_id.clone();
        move || {
            state.with(|state| {
                let items = state.as_ref().and_then(|state| state.active_set.as_ref()).map(|set| &set.items);
                items.is_some_and(|items| items.iter().any(|item| item.song_id.as_ref() == Some(&song_id)))
            })
        }
    };
    let add_class = {
        let in_set = in_set.clone();
        move || if in_set() { "btn btn--quiet" } else { "btn btn--primary" }
    };
    let add_label = move || if in_set() { pl(Msg::AddAgain) } else { pl(Msg::AddToSetLong) };
    let (add_id, edit_id, show_id) = (song_id.clone(), song_id.clone(), song_id);
    let add = move |_| {
        if commands.try_send(Command::AddSong { song_id: add_id.clone() }) {
            toasts.show(format!("{}: {}", pl(Msg::Added), title()));
            open.set(None);
        }
    };
    let edit = move |_| {
        editing.set(Some(EditorTarget::Existing(edit_id.clone())));
        open.set(None);
    };
    let show = move |_| {
        if commands.try_send(Command::ShowNow { song_id: show_id.clone() }) {
            open.set(None);
        }
    };
    view! {
        <Sheet title=pl(Msg::SongCard).to_owned() on_close=close testid="song-card">
            <h3 class="card-title">{title} {move || number().map(|number| format!(" · {number}"))}</h3>
            <div class="card-slides">{slides}</div>
            <div class="sheet-actions">
                <button class="btn btn--quiet" data-testid="card-show-now" on:click=show>
                    {pl(Msg::ShowNowOnScreen)}
                </button>
                <button class="btn" data-testid="card-edit" on:click=edit>
                    {pl(Msg::Edit)}
                </button>
                <button class=add_class data-testid="card-add" on:click=add>
                    {add_label}
                </button>
            </div>
        </Sheet>
    }
}
