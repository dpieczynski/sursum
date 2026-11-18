// SPDX-License-Identifier: GPL-3.0-or-later
//! The slides of the live item as tiles with the section name and the first line, so the
//! organist can jump to "Refren" or "Zwrotka 3" by what it says, not by a number. A tap shows
//! the slide (and lifts black). The current tile is kept second in the list, so the slide before
//! it and the ones after it stay in view; it is green only while the congregation sees it.

use leptos::prelude::*;
use sursum_protocol::{Command, RemoteState, Slide};

use crate::Commands;
use crate::live::info::{screen_shows, slide_first_line, slide_name};
use crate::messages::{Msg, pl};

#[component]
pub fn Slides(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let container = NodeRef::<leptos::html::Div>::new();
    let current = Signal::derive(move || {
        state.with(|state| {
            let live = state.as_ref()?.program.live.as_ref()?;
            (!live.gap).then_some(live.slide_index)
        })
    });
    let on_air = Signal::derive(move || {
        state.with(|state| state.as_ref().is_some_and(|state| screen_shows(state).is_content()))
    });
    let tiles = move || {
        let slides = state.with(|state| {
            state.as_ref().and_then(|state| state.program.live.as_ref().map(|live| live.deck.slides.clone()))
        });
        slides
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, slide)| tile(index as u32, slide, current, on_air, commands))
            .collect_view()
    };
    Effect::new(move |_| {
        current.track();
        crate::scroll::reveal_after_render(container, ".slide-tile[aria-current]");
    });
    view! {
        <div class="slides" node_ref=container data-testid="slides">
            {tiles}
        </div>
    }
}

fn tile(
    index: u32,
    slide: Slide,
    current: Signal<Option<u32>>,
    on_air: Signal<bool>,
    commands: Commands,
) -> impl IntoView {
    let name = slide_name(&slide).unwrap_or_else(|| format!("{} {}", pl(Msg::SlideN), index + 1));
    let line = slide_first_line(&slide);
    let is_current = move || current.get() == Some(index);
    let connection = expect_context::<crate::Connection>();
    let show = move |_| commands.send(Command::GoToSlide { slide_index: index });
    view! {
        <button
            class="slide-tile tappable"
            class:current=move || is_current() && on_air.get()
            class:covered=move || is_current() && !on_air.get()
            aria-current=move || is_current().then_some("true")
            prop:disabled=move || !connection.up()
            on:click=show
        >
            <span class="slide-name">{name}</span>
            <span class="slide-line">{line}</span>
        </button>
    }
}
