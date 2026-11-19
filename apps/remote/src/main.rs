// SPDX-License-Identifier: GPL-3.0-or-later
//! The operator's remote: three views (Msza for running the service with one hand, Zestaw for
//! preparing it, Ustawienia for the rest) around a live area that never moves.
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
mod import;
mod key_settings;
mod keys;
mod live;
mod maintenance;
mod messages;
mod pairing;
mod picture;
mod power;
mod prefs;
mod scroll;
mod search;
mod sets;
mod settings;
mod shell;
mod songs;
mod ui;

use leptos::prelude::*;
use messages::{Msg, pl};
use shell::{CurrentView, View};
use sursum_client::{Client, needs_reload};
use sursum_protocol::{ClientMsg, CmdOutcome, Command, RemoteState, Role, ServerMsg};

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
        let open = sursum_protocol::SessionInfo { paired: true, admin: true, ..Default::default() };
        session.set(Some(info.unwrap_or(open)));
    });
    move || match session.get() {
        None => view! { <p class="loading">{pl(Msg::Connecting)}</p> }.into_any(),
        Some(info) if info.setup_needed => view! { <pairing::Setup /> }.into_any(),
        Some(info) if !info.paired => view! { <pairing::Pairing /> }.into_any(),
        Some(info) => view! { <App info /> }.into_any(),
    }
}

/// The connection status (shared through the context): controls are disabled without it.
#[derive(Clone, Copy)]
pub struct Connection(pub RwSignal<sursum_client::Status>);

impl Connection {
    /// True while commands can reach the server.
    pub fn up(self) -> bool {
        self.0.get() == sursum_client::Status::Connected
    }
}

/// Sends commands with the revision the operator currently sees (for the `Stale` rule).
#[derive(Clone, Copy)]
pub struct Commands {
    client: StoredValue<Client, LocalStorage>,
    state: RwSignal<Option<RemoteState>>,
    seq: StoredValue<u32>,
    toasts: ui::Toasts,
}

impl Commands {
    /// Sends a command; without a connection the operator gets a toast instead.
    pub fn send(self, cmd: Command) {
        let _ = self.try_send(cmd);
    }

    /// Sends a command; false (and a toast) if there is no connection. Commands are never queued:
    /// a "next" replayed after a reconnect would move the screen unexpectedly.
    pub fn try_send(self, cmd: Command) -> bool {
        let base_rev =
            self.state.with_untracked(|state| state.as_ref().map(|state| state.rev).unwrap_or_default());
        self.seq.update_value(|seq| *seq = seq.wrapping_add(1));
        let message = ClientMsg::Cmd { seq: self.seq.get_value(), base_rev, cmd };
        let sent = self.client.with_value(|client| client.send(&message));
        if !sent {
            self.toasts.show(pl(Msg::NotSent));
        }
        sent
    }
}

#[component]
fn App(info: sursum_protocol::SessionInfo) -> impl IntoView {
    let state = RwSignal::new(None::<RemoteState>);
    let toasts = ui::Toasts::default();
    provide_context(toasts);
    let session = admin::Session(RwSignal::new(info));
    provide_context(session);

    let client = Client::connect(Role::Remote, move |message| match message {
        ServerMsg::Welcome { .. } if needs_reload(&message) => reload(),
        ServerMsg::Remote { state: snapshot } => state.set(Some(snapshot)),
        ServerMsg::CmdResult { outcome: CmdOutcome::Stale { .. }, .. } => {
            toasts.show(pl(Msg::AlreadyAdvanced));
        }
        ServerMsg::CmdResult { outcome: CmdOutcome::Rejected { code }, .. } => {
            toasts.show(pl(messages::rejection(code)));
            if code == sursum_protocol::ErrorCode::Forbidden {
                // Perhaps admin mode expired: show the right tools again.
                session.refresh();
            }
        }
        _ => {}
    });
    let status = client.status();
    provide_context(Connection(status));
    let commands =
        Commands { client: StoredValue::new_local(client), state, seq: StoredValue::new(0), toasts };
    provide_context(commands);
    let prefs = prefs::Prefs::load();
    provide_context(prefs);
    let keys = key_settings::Keys { map: RwSignal::new(keys::KeyMap::load()), learning: RwSignal::new(None) };
    provide_context(keys);
    key_settings::listen(keys, commands);
    let editing = RwSignal::new(None::<editor::EditorTarget>);
    provide_context(editing);
    let calibrating = calibration::Calibrating(RwSignal::new(false));
    provide_context(calibrating);
    provide_context(info.installation);
    let current = CurrentView::new(info.installation);
    provide_context(current);
    // Back and forward (also Android's back gesture) move between the views they recorded.
    let history = window_event_listener(leptos::ev::hashchange, move |_| current.follow_location());
    on_cleanup(move || history.remove());
    let maintenance = maintenance::Maintenance::new();
    provide_context(maintenance);

    let open = calibrating.0;
    let calibration_view =
        move || open.get().then(|| view! { <calibration::CalibrationPanel open state commands /> });
    let searching = RwSignal::new(false);
    let search_sheet =
        move || searching.get().then(|| view! { <live::SearchSheet commands open=searching /> });
    let screen_open = RwSignal::new(false);
    let screen_sheet =
        move || screen_open.get().then(|| view! { <live::ScreenSheet state commands open=screen_open /> });
    let content = move || match (editing.get(), current.view.get()) {
        (Some(target), _) => view! { <editor::Editor target editing state commands /> }.into_any(),
        (None, View::Live) => view! { <live::LiveSet state commands /> }.into_any(),
        (None, View::Set) => view! { <sets::SetsView state commands /> }.into_any(),
        (None, View::Settings) => view! { <settings::SettingsView state commands /> }.into_any(),
        (None, View::Songs) => view! { <songs::SongsView /> }.into_any(),
    };
    let view_attr = move || current.view.get().attribute();
    let installation_attr = match info.installation {
        sursum_protocol::Installation::Screen => "screen",
        sursum_protocol::Installation::Library => "library",
    };
    let editing_attr = move || editing.with(Option::is_some).to_string();

    view! {
        {calibration_view}
        <main
            class="remote"
            data-view=view_attr
            data-installation=installation_attr
            data-hand=shell::hand_attribute(prefs)
            data-editing=editing_attr
        >
            <div class="area-top">
                <shell::TopBar status state on_status=move || screen_open.set(true) />
                <shell::Banners status state commands />
            </div>
            <section class="area-now">
                <live::Now state />
            </section>
            <section class="area-slides">
                <live::Slides state commands />
            </section>
            <section class="area-content">{content}</section>
            <section class="area-pad">
                <live::Pad state commands on_search=move || searching.set(true) />
            </section>
            <section class="area-strip">
                <live::LiveStrip state commands />
            </section>
        </main>
        <ui::Toast toasts send=move |command| commands.send(command) />
        <maintenance::MaintenanceSheet connection=status maintenance />
        {search_sheet}
        {screen_sheet}
    }
}

/// Reloads the page (a new server build, or data replaced from a backup).
pub(crate) fn reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}
