// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings of this tablet or PC, not of the device: which side the controls are on (it depends
//! on where the tablet stands on the console) and whether presses vibrate. Kept in the browser,
//! like the pedal keys.

use leptos::prelude::*;

const HAND_KEY: &str = "sursum.hand";
const VIBRATION_KEY: &str = "sursum.vibration";

/// Which side "Dalej" is on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Hand {
    #[default]
    Right,
    Left,
}

impl Hand {
    /// The value of `data-hand` on the page (the stylesheet mirrors the controls).
    pub fn attribute(self) -> &'static str {
        match self {
            Hand::Right => "right",
            Hand::Left => "left",
        }
    }

    fn from_attribute(value: &str) -> Hand {
        if value == "left" { Hand::Left } else { Hand::Right }
    }
}

/// This device's settings (shared through the context).
#[derive(Clone, Copy)]
pub struct Prefs {
    pub hand: RwSignal<Hand>,
    pub vibration: RwSignal<bool>,
}

impl Prefs {
    /// Reads the stored settings (defaults if none or no storage).
    pub fn load() -> Prefs {
        let stored = |key: &str| storage().and_then(|storage| storage.get_item(key).ok().flatten());
        let hand = stored(HAND_KEY).map_or(Hand::Right, |value| Hand::from_attribute(&value));
        let vibration = stored(VIBRATION_KEY).is_none_or(|value| value != "off");
        Prefs { hand: RwSignal::new(hand), vibration: RwSignal::new(vibration) }
    }

    /// Puts the controls on `hand`'s side and remembers it.
    pub fn set_hand(self, hand: Hand) {
        self.hand.set(hand);
        store(HAND_KEY, hand.attribute());
    }

    /// Turns vibration on or off and remembers it.
    pub fn set_vibration(self, on: bool) {
        self.vibration.set(on);
        store(VIBRATION_KEY, if on { "on" } else { "off" });
    }

    /// A short vibration as feedback for a control press (Android; ignored elsewhere).
    pub fn buzz(self) {
        if self.vibration.get_untracked()
            && let Some(window) = web_sys::window()
        {
            let _ = window.navigator().vibrate_with_duration(12);
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn store(key: &str, value: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(key, value);
    }
}
