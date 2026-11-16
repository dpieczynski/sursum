// SPDX-License-Identifier: GPL-3.0-or-later
//! The song editor: OpenSong markup typed on a keyboard, with slides previewed by the server
//! and drawn by the same component as the screen.

use leptos::ev;
use leptos::prelude::*;
use sursum_protocol::{
    Command, DraftPreview, LiveSlide, RemoteState, SaveOutcome, ScreenLayout, SongDraft, SongId, Theme,
};
use sursum_slide::{PreviewData, SlidePreview};

use crate::Commands;
use crate::api::{get_json, post_json};
use crate::messages::{Msg, pl};

/// Which song the editor works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorTarget {
    New,
    Existing(SongId),
}

/// Shown for a new song: the markup in a nutshell.
const NEW_SONG_TEMPLATE: &str = "[V1]\n \n\n[C]\n ";

/// Wait after the last keystroke before asking the server for a preview.
const PREVIEW_DEBOUNCE_MS: u32 = 300;

#[component]
pub fn Editor(
    target: EditorTarget,
    editing: RwSignal<Option<EditorTarget>>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
) -> impl IntoView {
    let draft = RwSignal::new(SongDraft { lyrics: NEW_SONG_TEMPLATE.to_owned(), ..SongDraft::default() });
    let status = RwSignal::new(None::<String>);
    if let EditorTarget::Existing(song_id) = target {
        leptos::task::spawn_local(async move {
            let url = format!("/api/songs/{}/draft", js_sys::encode_uri_component(song_id.as_str()));
            match get_json::<SongDraft>(&url).await {
                Some(loaded) => draft.set(loaded),
                None => status.set(Some(pl(Msg::LoadFailed).to_owned())),
            }
        });
    }
    let save = move |show_after: bool| {
        leptos::task::spawn_local(async move {
            let outcome = post_json::<_, SaveOutcome>("/api/songs/save", &draft.get_untracked()).await;
            status.set(Some(save_message(outcome.as_ref())));
            if let Some(SaveOutcome::Saved { song_id, revision }) = outcome {
                draft.update(|draft| {
                    draft.song_id = Some(song_id.clone());
                    draft.base_revision = Some(revision);
                });
                if show_after {
                    commands.send(Command::ShowNow { song_id });
                    editing.set(None);
                }
            }
        });
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
            "Escape" => editing.set(None),
            _ => {}
        }
    };
    view! {
        <div class="editor" on:keydown=on_keydown>
            <EditorForm draft status save editing />
            <EditorPreview draft state />
        </div>
    }
}

/// The text fields and buttons.
#[component]
fn EditorForm(
    draft: RwSignal<SongDraft>,
    status: RwSignal<Option<String>>,
    save: impl Fn(bool) + Copy + 'static,
    editing: RwSignal<Option<EditorTarget>>,
) -> impl IntoView {
    let field = move |update: fn(&mut SongDraft, String)| {
        move |event| {
            let value = event_target_value(&event);
            draft.update(|draft| update(draft, value));
        }
    };
    let title = move || draft.with(|draft| draft.title.clone());
    // Deleting needs admin mode and an existing song; the server checks both again.
    let session = expect_context::<crate::admin::Session>();
    let commands = expect_context::<crate::Commands>();
    let can_delete = move || session.0.get().admin && draft.with(|draft| draft.song_id.is_some());
    let delete = move || {
        if let Some(song_id) = draft.get_untracked().song_id {
            commands.send(sursum_protocol::Command::DeleteSong { song_id });
            editing.set(None);
        }
    };
    let number = move || draft.with(|draft| draft.number.clone().unwrap_or_default());
    let order = move || draft.with(|draft| draft.presentation.clone());
    let lyrics = move || draft.with(|draft| draft.lyrics.clone());
    view! {
        <form class="editor-form" on:submit=move |event| event.prevent_default()>
            <label>{pl(Msg::EditorTitle)}<input prop:value=title on:input=field(|d, v| d.title = v) /></label>
            <div class="editor-row">
                <label>
                    {pl(Msg::EditorNumber)}
                    <input prop:value=number on:input=field(|d, v| d.number = Some(v)) />
                </label>
                <label class="grow">
                    {pl(Msg::EditorOrder)}<input prop:value=order on:input=field(|d, v| d.presentation = v) />
                </label>
            </div>
            <label class="grow">
                {pl(Msg::EditorLyrics)}
                <textarea
                    class="lyrics"
                    spellcheck="false"
                    prop:value=lyrics
                    on:input=field(|d, v| d.lyrics = v)
                ></textarea>
            </label>
            <p class="editor-help">{pl(Msg::EditorHelp)}</p>
            <div class="editor-buttons">
                <button type="button" on:click=move |_| editing.set(None)>
                    {pl(Msg::Close)}
                </button>
                <Show when=can_delete>
                    <crate::ui::HoldButton label=pl(Msg::DeleteSong) on_hold=delete />
                </Show>
                <button type="button" on:click=move |_| save(false)>
                    {pl(Msg::Save)}
                </button>
                <button type="button" class="primary" on:click=move |_| save(true)>
                    {pl(Msg::SaveAndShow)}
                </button>
            </div>
            <p class="editor-status">{move || status.get()}</p>
        </form>
    }
}

/// Slides of the draft as the screen would show them, plus markup warnings.
#[component]
fn EditorPreview(draft: RwSignal<SongDraft>, state: RwSignal<Option<RemoteState>>) -> impl IntoView {
    let preview = RwSignal::new(None::<DraftPreview>);
    let generation = StoredValue::new(0_u32);
    Effect::new(move |_| {
        let current = draft.get();
        generation.update_value(|value| *value += 1);
        let this_request = generation.get_value();
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(PREVIEW_DEBOUNCE_MS).await;
            if generation.get_value() != this_request {
                return;
            }
            let result = post_json::<_, DraftPreview>("/api/songs/preview", &current).await;
            if generation.get_value() == this_request {
                preview.set(result);
            }
        });
    });
    let look = move || {
        state.with(|state| match state {
            Some(state) => (state.program.theme.clone(), state.program.layout.clone()),
            None => {
                (Theme::default(), ScreenLayout { content: sursum_protocol::Rect::SCENE, masks: Vec::new() })
            }
        })
    };
    let warnings =
        move || preview.with(|preview| preview.as_ref().map(|p| p.warnings.clone()).unwrap_or_default());
    let slides = move || {
        let Some(preview) = preview.get() else { return Vec::new() };
        let (theme, layout) = look();
        (0..preview.deck.slides.len())
            .map(|index| {
                let live = LiveSlide { deck: preview.deck.clone(), slide_index: index as u32, gap: false };
                PreviewData { live, theme: theme.clone(), layout: layout.clone() }
            })
            .collect::<Vec<_>>()
    };
    let slide_views = move || {
        slides()
            .into_iter()
            .map(|data| {
                view! {
                    <div class="editor-slide">
                        <SlidePreview data=Signal::stored(Some(data)) />
                    </div>
                }
            })
            .collect_view()
    };
    let warning_views = move || warnings().into_iter().map(|text| view! { <li>{text}</li> }).collect_view();
    view! {
        <section class="editor-preview">
            <ul class="editor-warnings">{warning_views}</ul>
            <div class="editor-slides">{slide_views}</div>
        </section>
    }
}

/// The status line after saving.
fn save_message(outcome: Option<&SaveOutcome>) -> String {
    match outcome {
        Some(SaveOutcome::Saved { .. }) => pl(Msg::Saved).to_owned(),
        Some(SaveOutcome::Conflict { current_revision }) => {
            format!("{} ({current_revision})", pl(Msg::Conflict))
        }
        Some(SaveOutcome::Invalid { reason }) if reason == "title_missing" => {
            pl(Msg::TitleMissing).to_owned()
        }
        Some(SaveOutcome::Invalid { reason }) if reason == "lyrics_missing" => {
            pl(Msg::LyricsMissing).to_owned()
        }
        Some(SaveOutcome::Invalid { .. }) | None => pl(Msg::SaveFailed).to_owned(),
    }
}
