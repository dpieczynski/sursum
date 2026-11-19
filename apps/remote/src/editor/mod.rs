// SPDX-License-Identifier: GPL-3.0-or-later
//! The song editor. Plain text is enough (verses separated by an empty line, "Ref." for the
//! refrain); OpenSong markup works too. The server turns either into slides and previews them;
//! the browser never interprets the text. Closing with unsaved changes asks first.

mod form;
mod preview;

use leptos::ev;
use leptos::prelude::*;
use sursum_protocol::{Command, RemoteState, SaveOutcome, SongDraft, SongId};

use crate::Commands;
use crate::api::{get_json, post_json};
use crate::messages::{Msg, pl};
use crate::ui::Sheet;

/// Which song the editor works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorTarget {
    /// A new song, with the title typed into the search if there was one.
    New {
        title: String,
    },
    Existing(SongId),
}

/// The parts of a draft the operator edits; compared to know whether something is unsaved.
fn edited_fields(draft: &SongDraft) -> (&str, Option<&str>, &str, &str) {
    (&draft.title, draft.number.as_deref(), &draft.presentation, &draft.lyrics)
}

#[component]
pub fn Editor(
    target: EditorTarget,
    editing: RwSignal<Option<EditorTarget>>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
) -> impl IntoView {
    let title = match &target {
        EditorTarget::New { title } => title.clone(),
        EditorTarget::Existing(_) => String::new(),
    };
    let draft = RwSignal::new(SongDraft { title, ..SongDraft::default() });
    // What was last loaded or saved, to tell whether closing would lose anything.
    let saved = RwSignal::new(draft.get_untracked());
    let status = RwSignal::new(None::<String>);
    if let EditorTarget::Existing(song_id) = target {
        leptos::task::spawn_local(async move {
            let url = format!("/api/songs/{}/draft", js_sys::encode_uri_component(song_id.as_str()));
            match get_json::<SongDraft>(&url).await {
                Some(loaded) => {
                    saved.set(loaded.clone());
                    draft.set(loaded);
                }
                None => status.set(Some(pl(Msg::LoadFailed).to_owned())),
            }
        });
    }
    let dirty = move || draft.with(|draft| saved.with(|saved| edited_fields(draft) != edited_fields(saved)));
    let save = move |show_after: bool| {
        leptos::task::spawn_local(async move {
            let outcome = post_json::<_, SaveOutcome>("/api/songs/save", &draft.get_untracked()).await;
            status.set(Some(save_message(outcome.as_ref())));
            if let Some(SaveOutcome::Saved { song_id, version }) = outcome {
                draft.update(|draft| {
                    draft.song_id = Some(song_id.clone());
                    draft.base_version = Some(version);
                });
                saved.set(draft.get_untracked());
                if show_after {
                    commands.send(Command::ShowNow { song_id });
                    editing.set(None);
                }
            }
        });
    };
    let discarding = RwSignal::new(false);
    let close = move || {
        if dirty() {
            discarding.set(true);
        } else {
            editing.set(None);
        }
    };
    let on_keydown = move |event: ev::KeyboardEvent| {
        let shortcut = event.ctrl_key() || event.meta_key();
        match event.key().as_str() {
            "s" | "S" if shortcut => {
                event.prevent_default();
                save(false);
            }
            "Enter" if shortcut => {
                event.prevent_default();
                save(true);
            }
            "Escape" if !discarding.get_untracked() => close(),
            _ => {}
        }
    };
    let discard_sheet = move || {
        discarding.get().then(|| {
            view! {
                <Sheet
                    title=pl(Msg::DiscardTitle).to_owned()
                    on_close=move || discarding.set(false)
                    testid="discard"
                >
                    <p>{pl(Msg::DiscardHint)}</p>
                    <div class="sheet-actions">
                        <button class="btn" on:click=move |_| discarding.set(false)>
                            {pl(Msg::KeepEditing)}
                        </button>
                        <button
                            class="btn btn--danger"
                            data-testid="discard-confirm"
                            on:click=move |_| editing.set(None)
                        >
                            {pl(Msg::Discard)}
                        </button>
                    </div>
                </Sheet>
            }
        })
    };
    view! {
        <div class="editor" on:keydown=on_keydown>
            <form::EditorForm draft status save close />
            <preview::EditorPreview draft state />
        </div>
        {discard_sheet}
    }
}

fn save_message(outcome: Option<&SaveOutcome>) -> String {
    match outcome {
        Some(SaveOutcome::Saved { .. }) => pl(Msg::Saved).to_owned(),
        Some(SaveOutcome::Conflict) => pl(Msg::Conflict).to_owned(),
        Some(SaveOutcome::Invalid { reason }) if reason == "title_missing" => {
            pl(Msg::TitleMissing).to_owned()
        }
        Some(SaveOutcome::Invalid { reason }) if reason == "lyrics_missing" => {
            pl(Msg::LyricsMissing).to_owned()
        }
        Some(SaveOutcome::Invalid { .. }) | None => pl(Msg::SaveFailed).to_owned(),
    }
}
