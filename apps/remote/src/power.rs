// SPDX-License-Identifier: GPL-3.0-or-later
//! Display power (ADR 0012) in words. The buttons are in the "Ekran" sheet behind the status
//! chip ("on" with one tap, "standby" with a long hold) and a banner offers "on" while the display
//! is asleep.

use sursum_protocol::DisplayPowerState;

use crate::messages::Msg;

/// The text for a power state.
pub fn state_label(state: &DisplayPowerState) -> Msg {
    match state {
        DisplayPowerState::Unknown => Msg::DisplayUnknown,
        DisplayPowerState::On => Msg::DisplayIsOn,
        DisplayPowerState::Standby => Msg::DisplayIsStandby,
        DisplayPowerState::WarmingUp => Msg::DisplayWarming,
        DisplayPowerState::CoolingDown => Msg::DisplayCooling,
        DisplayPowerState::Unavailable { .. } => Msg::DisplayNoAnswer,
    }
}
