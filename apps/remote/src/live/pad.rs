// SPDX-License-Identifier: GPL-3.0-or-later
//! The controls in the thumb zone. "Dalej" is the biggest, on the hand's side, and says what it
//! will do; "Wygaś ekran" is in the opposite corner and turns into "Pokaż tekst" while black or
//! the logo is up. Positions never change, whatever the state; impossible steps are disabled.

use leptos::prelude::*;
use sursum_protocol::{Command, Overlay, RemoteState};

use crate::live::info::{Tone, can_go_back, item_steps, next_caption};
use crate::messages::{Msg, pl};
use crate::prefs::Prefs;
use crate::{Commands, Connection};

/// Whether the black/logo cover is up.
pub fn covered(state: RwSignal<Option<RemoteState>>) -> impl Fn() -> Option<Overlay> + Copy {
    move || {
        state.with(|state| {
            state.as_ref().map(|state| state.program.overlay).filter(|overlay| *overlay != Overlay::None)
        })
    }
}

/// "Wygaś ekran" or, while covered, "Pokaż tekst". Used by the pad and the live strip.
#[component]
pub fn CoverButton(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    #[prop(default = "btn--lg")] size: &'static str,
) -> impl IntoView {
    let prefs = expect_context::<Prefs>();
    let connection = expect_context::<Connection>();
    let covered = covered(state);
    let press = move |_| {
        let overlay = if covered().is_some() { Overlay::None } else { Overlay::Black };
        commands.send(Command::SetOverlay { overlay });
        prefs.buzz();
    };
    let label = move || if covered().is_some() { pl(Msg::ShowText) } else { pl(Msg::BlackOut) };
    view! {
        <button
            class=format!("btn {size} cover")
            class:revealing=move || covered().is_some()
            data-testid="black"
            aria-pressed=move || covered().is_some().to_string()
            prop:disabled=move || !connection.up()
            on:click=press
        >
            {label}
        </button>
    }
}

/// "Dalej" with the caption saying what it will do.
#[component]
pub fn NextButton(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    #[prop(default = "pad-next")] class: &'static str,
) -> impl IntoView {
    let prefs = expect_context::<Prefs>();
    let connection = expect_context::<Connection>();
    let caption = Memo::new(move |_| state.with(|state| state.as_ref().map(next_caption)));
    let enabled = move || caption.with(|caption| caption.as_ref().is_some_and(|caption| caption.enabled));
    let warn =
        move || caption.with(|caption| caption.as_ref().is_some_and(|caption| caption.tone == Tone::Warn));
    let text = move || caption.with(|caption| caption.as_ref().map(|caption| caption.text.clone()));
    let press = move |_| {
        commands.send(Command::Next);
        prefs.buzz();
    };
    view! {
        <button
            class=format!("btn btn--primary {class}")
            class:leaving=warn
            data-testid="next"
            prop:disabled=move || !enabled() || !connection.up()
            on:click=press
        >
            <span class="next-label">{pl(Msg::Next)} " ▶"</span>
            <span class="next-caption" data-testid="next-caption">
                {text}
            </span>
        </button>
    }
}

/// "Wstecz".
#[component]
pub fn PrevButton(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    size: &'static str,
) -> impl IntoView {
    let prefs = expect_context::<Prefs>();
    let connection = expect_context::<Connection>();
    let enabled = move || connection.up() && state.with(|state| state.as_ref().is_some_and(can_go_back));
    let press = move |_| {
        commands.send(Command::Prev);
        prefs.buzz();
    };
    view! {
        <button
            class=format!("btn {size} pad-prev")
            data-testid="prev"
            prop:disabled=move || !enabled()
            on:click=press
        >
            "◀ "
            {pl(Msg::Prev)}
        </button>
    }
}

#[component]
pub fn Pad(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    on_search: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let prefs = expect_context::<Prefs>();
    let connection = expect_context::<Connection>();
    let steps = move || {
        let steps = state.with(|state| state.as_ref().map_or((false, false), item_steps));
        if connection.up() { steps } else { (false, false) }
    };
    let logo_up = move || {
        state.with(|state| state.as_ref().is_some_and(|state| state.program.overlay == Overlay::Logo))
    };
    let send = move |command: Command| {
        commands.send(command);
        prefs.buzz();
    };
    let toggle_logo = move |_| {
        let overlay = if logo_up() { Overlay::None } else { Overlay::Logo };
        send(Command::SetOverlay { overlay });
    };
    let logo_label = move || if logo_up() { pl(Msg::LogoOnScreen) } else { pl(Msg::ShowLogo) };
    view! {
        <div class="pad" data-testid="pad">
            <button
                class="btn btn--sm pad-logo"
                class:active=logo_up
                data-testid="logo"
                aria-pressed=move || logo_up().to_string()
                prop:disabled=move || !connection.up()
                on:click=toggle_logo
            >
                {logo_label}
            </button>
            <button
                class="btn btn--sm pad-prev-item"
                data-testid="prev-item"
                prop:disabled=move || !steps().0
                on:click=move |_| send(Command::PrevItem)
            >
                {pl(Msg::PrevSong)}
            </button>
            <button
                class="btn btn--sm pad-next-item"
                data-testid="next-item"
                prop:disabled=move || !steps().1
                on:click=move |_| send(Command::NextItem)
            >
                {pl(Msg::NextSong)}
            </button>
            <button class="btn btn--sm pad-search" data-testid="search" on:click=move |_| on_search()>
                {pl(Msg::Search)}
            </button>
            <PrevButton state commands size="btn--lg" />
            <CoverButton state commands />
            <NextButton state commands />
        </div>
    }
}
