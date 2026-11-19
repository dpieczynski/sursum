// SPDX-License-Identifier: GPL-3.0-or-later
//! The editor's preview: the slides the server makes from the text (drawn like the screen), and
//! its warnings in words.

use leptos::prelude::*;
use sursum_protocol::{
    DraftPreview, LiveSlide, MarkupWarning, MarkupWarningCode, RemoteState, ScreenLayout, SongDraft,
};
use sursum_slide::{PreviewData, SlidePreview};

use crate::api::post_json;
use crate::messages::{Msg, pl};

/// Wait after the last keystroke before asking the server for a preview.
const PREVIEW_DEBOUNCE_MS: u32 = 300;

#[component]
pub fn EditorPreview(draft: RwSignal<SongDraft>, state: RwSignal<Option<RemoteState>>) -> impl IntoView {
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
            Some(state) => state.program.layout.clone(),
            None => ScreenLayout { content: sursum_protocol::Rect::SCENE, masks: Vec::new() },
        })
    };
    let slide_views = move || {
        let Some(preview) = preview.get() else { return Vec::new() };
        let layout = look();
        (0..preview.deck.slides.len())
            .map(|index| {
                let live = LiveSlide { deck: preview.deck.clone(), slide_index: index as u32, gap: false };
                let data = PreviewData { live, layout: layout.clone() };
                view! {
                    <div class="editor-slide">
                        <SlidePreview data=Signal::stored(Some(data)) />
                    </div>
                }
            })
            .collect::<Vec<_>>()
    };
    let warning_views = move || {
        preview
            .with(|preview| {
                preview
                    .as_ref()
                    .map(|preview| preview.warnings.iter().map(warning_text).collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .into_iter()
            .map(|text| view! { <li>{text}</li> })
            .collect_view()
    };
    view! {
        <section class="editor-preview">
            <ul class="editor-warnings">{warning_views}</ul>
            <div class="editor-slides">{slide_views}</div>
        </section>
    }
}

/// A warning in Polish, e.g. "Linia 3: sekcja [V1] występuje dwa razy – połączono".
fn warning_text(warning: &MarkupWarning) -> String {
    let text = match warning.code {
        MarkupWarningCode::LinesBeforeFirstSection => pl(Msg::WarnLinesBefore).to_owned(),
        MarkupWarningCode::DuplicateSection => {
            format!("{} [{}] {}", pl(Msg::WarnSection), warning.detail, pl(Msg::WarnTwice))
        }
        MarkupWarningCode::UnknownSectionInOrder => {
            format!("{} „{}”", pl(Msg::WarnUnknownInOrder), warning.detail)
        }
        MarkupWarningCode::Other => warning.detail.clone(),
    };
    match warning.line {
        Some(line) => format!("{} {line}: {text}", pl(Msg::Line)),
        None => text,
    }
}
