// SPDX-License-Identifier: GPL-3.0-or-later
//! The editor's form: title and number, the text with buttons that insert a verse, a refrain, a
//! bridge or a slide break at the cursor, a short "how to write" help, the order under
//! "Zaawansowane", and the actions.

use leptos::prelude::*;
use sursum_protocol::SongDraft;
use wasm_bindgen::JsCast;

use crate::messages::{Msg, pl};

/// What the insert buttons add, in plain text (or in markup if the text already uses it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Insert {
    Verse,
    Refrain,
    Bridge,
    SlideBreak,
}

impl Insert {
    fn label(self) -> &'static str {
        pl(match self {
            Insert::Verse => Msg::InsertVerse,
            Insert::Refrain => Msg::InsertRefrain,
            Insert::Bridge => Msg::InsertBridge,
            Insert::SlideBreak => Msg::InsertSlideBreak,
        })
    }

    /// The text to insert. Typing aid only: the server decides what it means.
    fn snippet(self, uses_markup: bool) -> &'static str {
        match (self, uses_markup) {
            (Insert::Verse, false) => "\n\n",
            (Insert::Refrain, false) => "\n\nRef. ",
            (Insert::Bridge, false) => "\n\nMost: ",
            (Insert::Verse, true) => "\n[V]\n",
            (Insert::Refrain, true) => "\n[C]\n",
            (Insert::Bridge, true) => "\n[B]\n",
            (Insert::SlideBreak, _) => "||",
        }
    }
}

/// `text` with `snippet` put in at the character `at` (clamped), and where the cursor goes.
fn inserted(text: &str, at: usize, snippet: &str) -> (String, usize) {
    let at = text.char_indices().nth(at).map_or(text.len(), |(byte, _)| byte);
    let mut out = String::with_capacity(text.len() + snippet.len());
    out.push_str(&text[..at]);
    out.push_str(snippet);
    out.push_str(&text[at..]);
    let cursor = out[..at + snippet.len()].chars().count();
    (out, cursor)
}

#[component]
pub fn EditorForm(
    draft: RwSignal<SongDraft>,
    status: RwSignal<Option<String>>,
    save: impl Fn(bool) + Copy + 'static,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let field = move |update: fn(&mut SongDraft, String)| {
        move |event| {
            let value = event_target_value(&event);
            draft.update(|draft| update(draft, value));
        }
    };
    let lyrics_ref = NodeRef::<leptos::html::Textarea>::new();
    let insert = move |what: Insert| {
        let Some(area) = lyrics_ref.get_untracked() else { return };
        let area: web_sys::HtmlTextAreaElement = area.unchecked_into();
        let text = area.value();
        let at = area.selection_start().ok().flatten().unwrap_or(text.chars().count() as u32) as usize;
        let uses_markup = text.lines().any(|line| line.trim_start().starts_with('['));
        let (updated, cursor) = inserted(&text, at, what.snippet(uses_markup));
        draft.update(|draft| draft.lyrics.clone_from(&updated));
        area.set_value(&updated);
        let _ = area.focus();
        let _ = area.set_selection_range(cursor as u32, cursor as u32);
    };
    let insert_buttons = [Insert::Verse, Insert::Refrain, Insert::Bridge, Insert::SlideBreak]
        .into_iter()
        .map(|what| {
            view! {
                <button type="button" class="btn btn--sm" on:click=move |_| insert(what)>
                    {what.label()}
                </button>
            }
        })
        .collect_view();
    let session = expect_context::<crate::admin::Session>();
    let commands = expect_context::<crate::Commands>();
    let editing = expect_context::<RwSignal<Option<super::EditorTarget>>>();
    let can_delete = move || session.0.get().admin && draft.with(|draft| draft.song_id.is_some());
    let delete = move || {
        if let Some(song_id) = draft.get_untracked().song_id {
            commands.send(sursum_protocol::Command::DeleteSong { song_id });
            editing.set(None);
        }
    };
    let title = move || draft.with(|draft| draft.title.clone());
    let number = move || draft.with(|draft| draft.number.clone().unwrap_or_default());
    let order = move || draft.with(|draft| draft.presentation.clone());
    let lyrics = move || draft.with(|draft| draft.lyrics.clone());
    view! {
        <form class="editor-form" on:submit=move |event| event.prevent_default()>
            <div class="editor-row">
                <label class="field grow">
                    {pl(Msg::EditorTitle)}
                    <input data-testid="editor-title" prop:value=title on:input=field(|d, v| d.title = v) />
                </label>
                <label class="field number">
                    {pl(Msg::EditorNumber)}
                    <input prop:value=number on:input=field(|d, v| d.number = Some(v)) />
                </label>
            </div>
            <div class="insert-buttons">{insert_buttons}</div>
            <label class="field grow">
                {pl(Msg::EditorLyrics)}
                <textarea
                    class="lyrics"
                    data-testid="editor-lyrics"
                    spellcheck="false"
                    placeholder=pl(Msg::LyricsPlaceholder)
                    node_ref=lyrics_ref
                    prop:value=lyrics
                    on:input=field(|d, v| d.lyrics = v)
                ></textarea>
            </label>
            <details class="editor-help">
                <summary>{pl(Msg::HowToWrite)}</summary>
                <ul>
                    <li>{pl(Msg::HelpVerses)}</li>
                    <li>{pl(Msg::HelpRefrain)}</li>
                    <li>{pl(Msg::HelpSlide)}</li>
                    <li>{pl(Msg::HelpOpenSong)}</li>
                    <li>{pl(Msg::HelpKeys)}</li>
                </ul>
            </details>
            <details class="editor-help">
                <summary>{pl(Msg::Advanced)}</summary>
                <label class="field">
                    {pl(Msg::EditorOrder)}
                    <input prop:value=order on:input=field(|d, v| d.presentation = v) />
                </label>
            </details>
            <div class="editor-actions">
                <button
                    type="button"
                    class="btn btn--sm btn--quiet"
                    data-testid="editor-close"
                    on:click=move |_| close()
                >
                    {pl(Msg::Close)}
                </button>
                <Show when=can_delete>
                    <crate::ui::HoldButton label=pl(Msg::DeleteSong) on_hold=delete />
                </Show>
                <button
                    type="button"
                    class="btn btn--sm"
                    data-testid="save-and-show"
                    on:click=move |_| save(true)
                >
                    {pl(Msg::SaveAndShow)}
                </button>
                <button
                    type="button"
                    class="btn btn--primary"
                    data-testid="save"
                    on:click=move |_| save(false)
                >
                    {pl(Msg::Save)}
                </button>
            </div>
            <p class="status-text" data-testid="editor-status">
                {move || status.get()}
            </p>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_go_in_at_the_cursor_counted_in_characters() {
        assert_eq!(inserted("ąb", 1, "||"), ("ą||b".to_owned(), 3));
        assert_eq!(inserted("ab", 9, "\n\n"), ("ab\n\n".to_owned(), 4), "past the end appends");
        assert_eq!(Insert::Refrain.snippet(false), "\n\nRef. ");
        assert_eq!(Insert::Refrain.snippet(true), "\n[C]\n");
    }
}
