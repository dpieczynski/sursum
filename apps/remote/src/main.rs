// SPDX-License-Identifier: GPL-3.0-or-later
//! The operator's remote: what is on screen, what comes next, and large controls for one hand.
//!
//! It sends commands and renders snapshots; all decisions are made by the server.

// Leptos components take their props by value; clippy's pass-by-value lint does not fit them.
#![allow(clippy::needless_pass_by_value)]

mod admin;
mod api;
mod bundle;
mod calibration;
mod calibration_form;
mod editor;
mod hold;
mod import;
mod key_settings;
mod keypad;
mod keys;
mod messages;
mod pairing;
mod picture;
mod power;
mod search;
mod set_list;

use leptos::prelude::*;
use messages::{Msg, pl, section_label};
use sursum_client::{Client, Status, needs_reload};
use sursum_protocol::{ClientMsg, CmdOutcome, Command, Overlay, RemoteState, Role, SectionJump, ServerMsg};
use sursum_slide::{PreviewData, SlidePreview};

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(Root);
}

/// Asks the server whether this browser may control the screen, then shows the remote or the
/// pairing form.
#[component]
fn Root() -> impl IntoView {
    let session = RwSignal::new(None::<sursum_protocol::SessionInfo>);
    leptos::task::spawn_local(async move {
        let info = api::get_json::<sursum_protocol::SessionInfo>("/api/session").await;
        // If the question itself fails, try to connect anyway; the server decides.
        let open = sursum_protocol::SessionInfo {
            pin_required: false,
            paired: true,
            admin: true,
            setup_needed: false,
        };
        session.set(Some(info.unwrap_or(open)));
    });
    move || match session.get() {
        None => view! { <p class="loading">{pl(Msg::Connecting)}</p> }.into_any(),
        Some(info) if info.setup_needed => view! { <pairing::Setup /> }.into_any(),
        Some(info) if !info.paired => view! { <pairing::Pairing /> }.into_any(),
        Some(info) => view! { <App info /> }.into_any(),
    }
}

/// Sends commands with the revision the operator currently sees (for the `Stale` rule).
#[derive(Clone, Copy)]
pub struct Commands {
    client: StoredValue<Client, LocalStorage>,
    state: RwSignal<Option<RemoteState>>,
    seq: StoredValue<u32>,
}

impl Commands {
    pub fn send(self, cmd: Command) {
        let base_rev =
            self.state.with_untracked(|state| state.as_ref().map(|state| state.rev).unwrap_or_default());
        self.seq.update_value(|seq| *seq = seq.wrapping_add(1));
        let message = ClientMsg::Cmd { seq: self.seq.get_value(), base_rev, cmd };
        self.client.with_value(|client| client.send(&message));
    }
}

#[component]
fn App(info: sursum_protocol::SessionInfo) -> impl IntoView {
    let state = RwSignal::new(None::<RemoteState>);
    let notice = RwSignal::new(None::<Msg>);
    let session = admin::Session(RwSignal::new(info));
    provide_context(session);

    let client = Client::connect(Role::Remote, move |message| match message {
        ServerMsg::Welcome { .. } if needs_reload(&message) => reload(),
        ServerMsg::Remote { state: snapshot } => state.set(Some(snapshot)),
        ServerMsg::CmdResult { outcome: CmdOutcome::Stale { .. }, .. } => {
            notice.set(Some(Msg::AlreadyAdvanced));
        }
        ServerMsg::CmdResult { outcome: CmdOutcome::Rejected { code }, .. } => {
            notice.set(Some(messages::rejection(code)));
            if code == sursum_protocol::ErrorCode::Forbidden {
                // Perhaps admin mode expired: show the right tools again.
                session.refresh();
            }
        }
        ServerMsg::CmdResult { outcome: CmdOutcome::Applied { .. }, .. } => notice.set(None),
        _ => {}
    });
    let status = client.status();
    let commands = Commands { client: StoredValue::new_local(client), state, seq: StoredValue::new(0) };
    provide_context(commands);
    let keys = key_settings::Keys { map: RwSignal::new(keys::KeyMap::load()), learning: RwSignal::new(None) };
    provide_context(keys);
    key_settings::listen(keys, commands);
    let editing = RwSignal::new(None::<editor::EditorTarget>);
    provide_context(editing);
    let editor_view =
        move || editing.get().map(|target| view! { <editor::Editor target editing state commands /> });
    let calibrating = calibration::Calibrating(RwSignal::new(false));
    provide_context(calibrating);
    let open = calibrating.0;
    let calibration_view =
        move || open.get().then(|| view! { <calibration::CalibrationPanel open state commands /> });

    view! {
        {editor_view}
        {calibration_view}
        <main class="remote">
            <TopBar status notice state />
            <power::DisplayPowerBar state commands />
            <Previews state />
            <SectionChips state commands />
            <Library state commands />
            <Controls state commands />
        </main>
    }
}

#[component]
fn TopBar(
    status: RwSignal<Status>,
    notice: RwSignal<Option<Msg>>,
    state: RwSignal<Option<RemoteState>>,
) -> impl IntoView {
    let status_text = move || {
        pl(match status.get() {
            Status::Connected => Msg::Connected,
            Status::Connecting => Msg::Connecting,
            Status::Disconnected => Msg::Disconnected,
        })
    };
    let set_name = move || {
        state
            .with(|state| state.as_ref().and_then(|s| s.active_set.as_ref().map(|set| set.name.clone())))
            .unwrap_or_else(|| pl(Msg::NoSet).to_owned())
    };
    // Only once a snapshot arrived: before that we do not know.
    let screen_missing =
        move || state.with(|state| state.as_ref().is_some_and(|state| !state.screen_connected));
    view! {
        <header class="topbar">
            <span class="status-dot" class:connected=move || status.get() == Status::Connected></span>
            <span>{status_text}</span>
            <span>" · "{set_name}</span>
            <span class="notice">{move || notice.get().map(pl)}</span>
            <Show when=screen_missing>
                <span class="notice" data-testid="screen-missing">
                    {pl(Msg::ScreenMissing)}
                </span>
            </Show>
            <a class="about" href="/licenses" target="_blank">
                {pl(Msg::About)}
            </a>
        </header>
    }
}

#[component]
fn Previews(state: RwSignal<Option<RemoteState>>) -> impl IntoView {
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
    let overlay =
        move || state.with(|state| state.as_ref().map(|state| state.program.overlay).unwrap_or_default());
    let dimmed = Signal::derive(move || overlay() != Overlay::None);
    let badge = move || match overlay() {
        Overlay::None => None,
        Overlay::Black => Some(pl(Msg::Black)),
        Overlay::Logo => Some(pl(Msg::Logo)),
    };
    let next_label = move || if next.with(Option::is_some) { pl(Msg::Next) } else { pl(Msg::EndOfSet) };
    view! {
        <section class="previews">
            <div class="preview-box live" data-testid="preview-live">
                <div class="preview-label">{pl(Msg::Live)}</div>
                <SlidePreview data=live dimmed />
                {move || badge().map(|text| view! { <span class="badge">{text}</span> })}
            </div>
            <div class="preview-box" data-testid="preview-next">
                <div class="preview-label">{next_label}</div>
                <SlidePreview data=next />
            </div>
        </section>
    }
}

#[component]
fn SectionChips(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let current = Signal::derive(move || {
        state.with(|state| state.as_ref().and_then(|s| s.program.live.as_ref().map(|live| live.slide_index)))
    });
    let chips = move || {
        let sections =
            state.with(|state| state.as_ref().map(|state| state.sections.clone()).unwrap_or_default());
        let ends: Vec<u32> =
            sections.iter().skip(1).map(|next| next.slide_index).chain(std::iter::once(u32::MAX)).collect();
        sections.into_iter().zip(ends).map(|(jump, end)| chip(jump, end, current, commands)).collect_view()
    };
    view! { <nav class="chips">{chips}</nav> }
}

/// One section button; highlighted while the live slide is inside its section.
fn chip(jump: SectionJump, end: u32, current: Signal<Option<u32>>, commands: Commands) -> impl IntoView {
    let start = jump.slide_index;
    let active = move || current.get().is_some_and(|index| index >= start && index < end);
    let jump_here = move |_| commands.send(Command::GoToSlide { slide_index: start });
    let label = section_label(&jump.label);
    view! {
        <button class="chip" class:active=active on:click=jump_here>
            {label}
        </button>
    }
}

/// The set, or search results while the search field is not empty.
#[component]
fn Library(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let searching = move || !query.with(|query| query.trim().is_empty());
    let set_list = move || view! { <set_list::SetPanel state commands /> };

    view! {
        <section class="library">
            <search::SearchBox query />
            <div class="library-tools">
                <NewSongButton />
                <bundle::ExportLink />
                <key_settings::KeysButton />
                <admin::AdminTools />
            </div>
            <Show when=searching fallback=set_list>
                <search::SearchResults query commands />
            </Show>
        </section>
    }
}

/// Opens the editor for a new song.
#[component]
fn NewSongButton() -> impl IntoView {
    let editing = expect_context::<RwSignal<Option<editor::EditorTarget>>>();
    view! {
        <button class="new-song" on:click=move |_| editing.set(Some(editor::EditorTarget::New))>
            {pl(Msg::NewSong)}
        </button>
    }
}

#[component]
fn Controls(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let overlay =
        move || state.with(|state| state.as_ref().map(|state| state.program.overlay).unwrap_or_default());
    let toggle_logo = move |_| {
        let target = if overlay() == Overlay::Logo { Overlay::None } else { Overlay::Logo };
        commands.send(Command::SetOverlay { overlay: target });
    };
    view! {
        <footer class="controls">
            <button class="prev-item" on:click=move |_| commands.send(Command::PrevItem)>
                {pl(Msg::PrevItem)}
            </button>
            <button class="next-item" on:click=move |_| commands.send(Command::NextItem)>
                {pl(Msg::NextItem)}
            </button>
            <button class="logo" class:active=move || overlay() == Overlay::Logo on:click=toggle_logo>
                {pl(Msg::Logo)}
            </button>
            <button class="next" data-testid="next" on:click=move |_| commands.send(Command::Next)>
                {pl(Msg::Next)}
            </button>
            <button class="prev" data-testid="prev" on:click=move |_| commands.send(Command::Prev)>
                {pl(Msg::Prev)}
            </button>
            <button
                class="black"
                data-testid="black"
                class:active=move || overlay() == Overlay::Black
                on:click=move |_| commands.send(Command::ToggleBlack)
            >
                {pl(Msg::Black)}
            </button>
            <keypad::JumpButton commands />
        </footer>
    }
}

fn reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}
