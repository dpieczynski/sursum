// SPDX-License-Identifier: GPL-3.0-or-later
//! "Teraz": the live item's title and position, the live preview (exactly what the screen
//! draws) and, where there is room, the next slide.

use leptos::prelude::*;
use sursum_protocol::RemoteState;
use sursum_slide::{PreviewData, SlidePreview};

use crate::live::info::{Shows, live_heading, live_position, screen_shows};
use crate::messages::{Msg, pl};

#[component]
pub fn Now(state: RwSignal<Option<RemoteState>>) -> impl IntoView {
    let live = Signal::derive(move || {
        state.with(|state| {
            let state = state.as_ref()?;
            let program = &state.program;
            Some(PreviewData {
                live: program.live.clone()?,
                theme: program.theme.clone(),
                layout: program.layout.clone(),
            })
        })
    });
    let next = Signal::derive(move || {
        state.with(|state| {
            let state = state.as_ref()?;
            Some(PreviewData {
                live: state.next.clone()?,
                theme: state.program.theme.clone(),
                layout: state.program.layout.clone(),
            })
        })
    });
    let shows = move || state.with(|state| state.as_ref().map_or(Shows::Nothing, screen_shows));
    let covered = Signal::derive(move || matches!(shows(), Shows::Black | Shows::Logo));
    let band = move || covered.get().then(|| view! { <span class="cover-band">{shows().label()}</span> });
    let heading = move || state.with(|state| state.as_ref().and_then(live_heading));
    let position = move || state.with(|state| state.as_ref().and_then(live_position));
    let nothing = move || heading().is_none();
    view! {
        <div class="now">
            <div class="now-text">
                <Show
                    when=nothing
                    fallback=move || {
                        view! {
                            <p class="now-title" data-testid="live-title">
                                {heading}
                            </p>
                            <p class="now-position" data-testid="live-position">
                                {position}
                            </p>
                        }
                    }
                >
                    <p class="now-empty">{pl(Msg::NothingLive)}</p>
                </Show>
            </div>
            <div class="now-previews">
                <div
                    class="preview-box live"
                    class:on-air=move || shows().is_content()
                    data-testid="preview-live"
                >
                    <SlidePreview data=live dimmed=covered />
                    {band}
                </div>
                <div class="preview-box next" data-testid="preview-next">
                    <span class="preview-label">{pl(Msg::NextSlide)}</span>
                    <SlidePreview data=next />
                </div>
            </div>
        </div>
    }
}
