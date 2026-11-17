// SPDX-License-Identifier: GPL-3.0-or-later
//! The frame around every view: tabs (Msza · Zestaw · Ustawienia) and the screen status in the
//! top bar, problem banners, and the regions the stylesheet arranges for phone, upright tablet
//! and wide screens. The live regions (what is on screen, the slides, the controls) stay put
//! when the tab changes; on narrow screens other tabs show the live strip instead. Problem
//! banners take a line under the top bar, so they push the live text down a little but never
//! the controls at the bottom.

use leptos::prelude::*;
use sursum_client::Status;
use sursum_protocol::{DisplayPowerState, RemoteState};

use crate::Commands;
use crate::live::info::screen_shows;
use crate::messages::{Msg, pl};
use crate::prefs::Prefs;
use crate::ui::{Banner, BannerKind};

/// The three views.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Live,
    Set,
    Settings,
}

impl View {
    const ALL: [View; 3] = [View::Live, View::Set, View::Settings];

    /// The URL fragment, so a reload (e.g. after an update) comes back to the same view.
    fn fragment(self) -> &'static str {
        match self {
            View::Live => "msza",
            View::Set => "zestaw",
            View::Settings => "ustawienia",
        }
    }

    fn label(self) -> &'static str {
        pl(match self {
            View::Live => Msg::TabLive,
            View::Set => Msg::TabSet,
            View::Settings => Msg::TabSettings,
        })
    }

    /// The view named in the URL fragment.
    pub fn from_location() -> View {
        let hash = web_sys::window().and_then(|window| window.location().hash().ok()).unwrap_or_default();
        View::ALL.into_iter().find(|view| hash.trim_start_matches('#') == view.fragment()).unwrap_or_default()
    }
}

/// The current view (shared through the context).
#[derive(Clone, Copy)]
pub struct CurrentView(pub RwSignal<View>);

impl CurrentView {
    /// Switches the view and records it in the URL fragment.
    pub fn show(self, view: View) {
        self.0.set(view);
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_hash(view.fragment());
        }
    }
}

/// The top bar: tabs on the left, the screen status on the right.
#[component]
pub fn TopBar(
    status: RwSignal<Status>,
    state: RwSignal<Option<RemoteState>>,
    on_status: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let current = expect_context::<CurrentView>();
    let tabs = View::ALL
        .into_iter()
        .map(|view| {
            let selected = move || current.0.get() == view;
            view! {
                <button
                    class="tab tappable"
                    class:selected=selected
                    role="tab"
                    aria-selected=move || selected().to_string()
                    data-testid=format!("tab-{}", view.fragment())
                    on:click=move |_| current.show(view)
                >
                    {view.label()}
                </button>
            }
        })
        .collect_view();
    let shows = move || state.with(|state| state.as_ref().map(screen_shows));
    let chip_text = move || shows().map_or(pl(Msg::Connecting), |shows| shows.label());
    let chip_short = move || shows().map_or(pl(Msg::Connecting), |shows| shows.short_label());
    let content = move || shows().is_some_and(|shows| shows.is_content());
    let connected = move || status.get() == Status::Connected;
    view! {
        <header class="topbar">
            <nav class="tabs" role="tablist">
                {tabs}
            </nav>
            <button
                class="status-chip tappable"
                class:content=content
                data-testid="screen-status"
                on:click=move |_| on_status()
            >
                <span
                    class="status-dot"
                    class:offline=move || !connected()
                    class:live=move || connected() && content()
                    data-testid=move || if connected() { "status-connected" } else { "status-offline" }
                ></span>
                <span class="status-chip-text long">{chip_text}</span>
                <span class="status-chip-text short">{chip_short}</span>
            </button>
        </header>
    }
}

/// The most serious problem the operator must see, if any: no connection, then no screen, then
/// the display asleep. One line under the top bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Problem {
    Disconnected,
    ScreenMissing,
    DisplayAsleep,
}

#[component]
pub fn Banners(
    status: RwSignal<Status>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
) -> impl IntoView {
    let problem = move || {
        if status.get() != Status::Connected {
            return Some(Problem::Disconnected);
        }
        // Only once a snapshot arrived: before that we do not know.
        state.with(|state| {
            let state = state.as_ref()?;
            if !state.screen_connected {
                Some(Problem::ScreenMissing)
            } else if state.display_power == DisplayPowerState::Standby {
                Some(Problem::DisplayAsleep)
            } else {
                None
            }
        })
    };
    let turn_on = move |_| {
        commands.send(sursum_protocol::Command::DisplayPower { action: sursum_protocol::PowerAction::On });
    };
    let banner = move || {
        problem().map(|problem| match problem {
            Problem::Disconnected => view! {
                <Banner kind=BannerKind::Danger testid="disconnected">
                    {pl(Msg::Disconnected)}
                </Banner>
            }
            .into_any(),
            Problem::ScreenMissing => view! {
                <Banner kind=BannerKind::Warn testid="screen-missing">
                    {pl(Msg::ScreenMissing)}
                </Banner>
            }
            .into_any(),
            Problem::DisplayAsleep => view! {
                <Banner kind=BannerKind::Warn testid="display-asleep">
                    <span>{pl(Msg::DisplayAsleep)}</span>
                    <button class="btn btn--sm" on:click=turn_on>
                        {pl(Msg::TurnOn)}
                    </button>
                </Banner>
            }
            .into_any(),
        })
    };
    view! { <div class="banners">{banner}</div> }
}

/// `data-hand` for the page.
pub fn hand_attribute(prefs: Prefs) -> impl Fn() -> &'static str + Copy {
    move || prefs.hand.get().attribute()
}
