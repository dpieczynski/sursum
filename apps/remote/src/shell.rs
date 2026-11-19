// SPDX-License-Identifier: GPL-3.0-or-later
//! The frame around every view: tabs (Msza · Zestaw · Ustawienia, or Pieśni · Ustawienia in the
//! desktop program's library) and the screen status in the top bar, problem banners, and the
//! regions the stylesheet arranges for phone, upright tablet and wide screens. The live regions (what is on screen, the slides, the controls) stay put
//! when the tab changes; on narrow screens other tabs show the live strip instead. Problem
//! banners take a line under the top bar, so they push the live text down a little but never
//! the controls at the bottom.

use leptos::prelude::*;
use sursum_client::Status;
use sursum_protocol::{DisplayPowerState, Installation, RemoteState};

use crate::Commands;
use crate::live::info::screen_shows;
use crate::messages::{Msg, pl};
use crate::prefs::Prefs;
use crate::ui::{Banner, BannerKind};

/// The views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Live,
    Set,
    Settings,
    /// The desktop program's library (no screen): search, new song, editor.
    Songs,
}

impl View {
    const ALL: [View; 4] = [View::Live, View::Set, View::Settings, View::Songs];

    /// The views an installation offers, in tab order; the first is the default.
    pub fn tabs(installation: Installation) -> &'static [View] {
        match installation {
            Installation::Screen => &[View::Live, View::Set, View::Settings],
            Installation::Library => &[View::Songs, View::Settings],
        }
    }

    /// The URL fragment, so a reload (e.g. after an update) comes back to the same view.
    fn fragment(self) -> &'static str {
        match self {
            View::Live => "msza",
            View::Set => "zestaw",
            View::Settings => "ustawienia",
            View::Songs => "piesni",
        }
    }

    /// `data-view` on the page, which the stylesheet arranges the regions by.
    pub fn attribute(self) -> &'static str {
        match self {
            View::Live => "live",
            View::Set => "set",
            View::Settings => "settings",
            View::Songs => "songs",
        }
    }

    fn label(self) -> &'static str {
        pl(match self {
            View::Live => Msg::TabLive,
            View::Set => Msg::TabSet,
            View::Settings => Msg::TabSettings,
            View::Songs => Msg::TabSongs,
        })
    }

    /// The view named in the URL fragment, if this installation offers it; else its first view.
    pub fn from_location(installation: Installation) -> View {
        let hash = web_sys::window().and_then(|window| window.location().hash().ok()).unwrap_or_default();
        let tabs = View::tabs(installation);
        View::ALL
            .into_iter()
            .find(|view| hash.trim_start_matches('#') == view.fragment() && tabs.contains(view))
            .unwrap_or(tabs[0])
    }
}

/// The current view (shared through the context).
#[derive(Clone, Copy)]
pub struct CurrentView {
    pub view: RwSignal<View>,
    pub installation: Installation,
}

impl CurrentView {
    /// Starts at the view named in the URL fragment.
    pub fn new(installation: Installation) -> Self {
        CurrentView { view: RwSignal::new(View::from_location(installation)), installation }
    }

    /// Follows the URL fragment (back and forward buttons).
    pub fn follow_location(self) {
        self.view.set(View::from_location(self.installation));
    }

    /// Switches the view and records it in the URL fragment (a history entry, so the browser's
    /// back button returns to the previous view; `App` follows the fragment when it changes).
    pub fn show(self, view: View) {
        self.view.set(view);
        if let Some(window) = web_sys::window()
            && View::from_location(self.installation) != view
        {
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
    let tabs = View::tabs(current.installation)
        .iter()
        .copied()
        .map(|view| {
            let selected = move || current.view.get() == view;
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
    // A library has no screen: only the connection can be a problem there.
    let has_screen = expect_context::<CurrentView>().installation == Installation::Screen;
    let problem = move || {
        if status.get() != Status::Connected {
            return Some(Problem::Disconnected);
        }
        if !has_screen {
            return None;
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
