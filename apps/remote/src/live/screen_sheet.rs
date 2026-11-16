// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Ekran" sheet behind the status chip: cover the screen, show the logo or the text, and
//! switch the display on or to standby where power control is configured (ADR 0012).

use leptos::prelude::*;
use sursum_protocol::{Command, DisplayPowerState, Overlay, PowerAction, RemoteState};

use crate::Commands;
use crate::live::info::screen_shows;
use crate::messages::{Msg, pl};
use crate::ui::{HoldButton, Sheet};

/// Standby needs a longer hold than other destructive actions.
const STANDBY_HOLD_MS: u32 = 1500;

#[component]
pub fn ScreenSheet(
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
    open: RwSignal<bool>,
) -> impl IntoView {
    let overlay = move |overlay: Overlay| {
        move |_| {
            commands.send(Command::SetOverlay { overlay });
            open.set(false);
        }
    };
    let shows = move || state.with(|state| state.as_ref().map(|state| screen_shows(state).label()));
    let power = move || {
        state.with(|state| state.as_ref().map(|state| state.display_power.clone()).unwrap_or_default())
    };
    let configured =
        move || !matches!(power(), DisplayPowerState::Unavailable { reason } if reason == "not_configured");
    let cooling = move || power() == DisplayPowerState::CoolingDown;
    let shows_content =
        move || state.with(|state| state.as_ref().is_some_and(|state| screen_shows(state).is_content()));
    let turn_on = move |_| commands.send(Command::DisplayPower { action: PowerAction::On });
    let standby = move || commands.send(Command::DisplayPower { action: PowerAction::Standby });
    view! {
        <Sheet title=pl(Msg::ScreenSheet).to_owned() on_close=move || open.set(false) testid="screen-sheet">
            <p class="sheet-status">{shows}</p>
            <div class="sheet-grid">
                <button class="btn btn--lg cover" on:click=overlay(Overlay::Black)>
                    {pl(Msg::BlackOut)}
                </button>
                <button class="btn btn--lg" on:click=overlay(Overlay::Logo)>
                    {pl(Msg::ShowLogo)}
                </button>
                <button class="btn btn--lg btn--live" on:click=overlay(Overlay::None)>
                    {pl(Msg::ShowText)}
                </button>
            </div>
            <Show when=configured>
                <div class="power" data-testid="display-power">
                    <span class="power-state">{move || pl(crate::power::state_label(&power()))}</span>
                    <button class="btn power-on" on:click=turn_on prop:disabled=cooling>
                        {pl(Msg::DisplayOn)}
                    </button>
                    <HoldButton
                        label=pl(Msg::DisplayStandby)
                        on_hold=standby
                        hold_ms=STANDBY_HOLD_MS
                        class="btn--md btn--danger"
                    />
                    <Show when=move || shows_content()>
                        <span class="hint warn">{pl(Msg::SlideOnScreen)}</span>
                    </Show>
                    <Show when=cooling>
                        <span class="hint">{pl(Msg::ProjectorCooling)}</span>
                    </Show>
                </div>
            </Show>
        </Sheet>
    }
}
