// SPDX-License-Identifier: GPL-3.0-or-later
//! Display power (ADR 0012): the projector's or monitor's state, "on" with one tap, "standby"
//! with a long hold (switching off during Mass would hurt), plus a warning while a slide is
//! showing. Hidden when no power control is configured.

use leptos::prelude::*;
use sursum_protocol::{Command, DisplayPowerState, Overlay, PowerAction, RemoteState};

use crate::Commands;
use crate::hold::HoldButton;
use crate::messages::{Msg, pl};

/// Standby needs a longer hold than other destructive actions.
const STANDBY_HOLD_MS: u32 = 1500;

/// State and buttons for the display's power.
#[component]
pub fn DisplayPowerBar(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let power = move || {
        state.with(|state| state.as_ref().map(|state| state.display_power.clone()).unwrap_or_default())
    };
    let configured =
        move || !matches!(power(), DisplayPowerState::Unavailable { reason } if reason == "not_configured");
    let content_on_screen = move || {
        state.with(|state| {
            state
                .as_ref()
                .is_some_and(|state| state.program.live.is_some() && state.program.overlay == Overlay::None)
        })
    };
    let cooling = move || power() == DisplayPowerState::CoolingDown;
    let turn_on = move |_| commands.send(Command::DisplayPower { action: PowerAction::On });
    let standby = move || commands.send(Command::DisplayPower { action: PowerAction::Standby });
    view! {
        <Show when=configured>
            <div class="power" data-testid="display-power">
                <span class="power-state">{move || pl(state_label(&power()))}</span>
                <button class="power-on" on:click=turn_on prop:disabled=cooling>
                    {pl(Msg::DisplayOn)}
                </button>
                <HoldButton label=pl(Msg::DisplayStandby) on_hold=standby hold_ms=STANDBY_HOLD_MS />
                <Show when=content_on_screen>
                    <span class="power-warning">{pl(Msg::SlideOnScreen)}</span>
                </Show>
                <Show when=cooling>
                    <span class="power-warning">{pl(Msg::ProjectorCooling)}</span>
                </Show>
            </div>
        </Show>
    }
}

fn state_label(state: &DisplayPowerState) -> Msg {
    match state {
        DisplayPowerState::Unknown => Msg::DisplayUnknown,
        DisplayPowerState::On => Msg::DisplayIsOn,
        DisplayPowerState::Standby => Msg::DisplayIsStandby,
        DisplayPowerState::WarmingUp => Msg::DisplayWarming,
        DisplayPowerState::CoolingDown => Msg::DisplayCooling,
        DisplayPowerState::Unavailable { .. } => Msg::DisplayNoAnswer,
    }
}
