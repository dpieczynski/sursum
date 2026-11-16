// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyboard, page-turner and pedal control (M5). A pedal or presentation clicker is a keyboard to
//! the browser, so every press is a key name like `PageDown`. Which key does what is kept in this
//! browser (`localStorage`), because it belongs to the tablet the pedal is paired with; a learn
//! mode lets the organist simply press the pedal to assign it.

use serde::{Deserialize, Serialize};
use sursum_protocol::Command;

/// Where the mapping is kept in the browser.
const STORAGE_KEY: &str = "sursum.keymap";

/// What a key can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Next,
    Prev,
    NextItem,
    PrevItem,
    ToggleBlack,
}

impl Action {
    /// Every action, in the order the settings show them.
    pub const ALL: [Action; 5] =
        [Action::Next, Action::Prev, Action::NextItem, Action::PrevItem, Action::ToggleBlack];

    /// The command this action sends.
    pub fn command(self) -> Command {
        match self {
            Action::Next => Command::Next,
            Action::Prev => Command::Prev,
            Action::NextItem => Command::NextItem,
            Action::PrevItem => Command::PrevItem,
            Action::ToggleBlack => Command::ToggleBlack,
        }
    }
}

/// Keys (as `KeyboardEvent.key` names them) and their actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMap {
    pub bindings: Vec<(String, Action)>,
}

impl Default for KeyMap {
    /// Common page turners send PageDown/PageUp or arrows; B and `.` are the usual black keys.
    fn default() -> Self {
        let bindings = [
            ("PageDown", Action::Next),
            ("ArrowRight", Action::Next),
            ("ArrowDown", Action::Next),
            (" ", Action::Next),
            ("Enter", Action::Next),
            ("PageUp", Action::Prev),
            ("ArrowLeft", Action::Prev),
            ("ArrowUp", Action::Prev),
            ("b", Action::ToggleBlack),
            ("B", Action::ToggleBlack),
            (".", Action::ToggleBlack),
        ];
        KeyMap { bindings: bindings.into_iter().map(|(key, action)| (key.to_owned(), action)).collect() }
    }
}

impl KeyMap {
    /// The action for a key. While typing in a field only PageUp/PageDown act (a pedal must work
    /// even with the search field focused; letters and arrows must not).
    pub fn action_for(&self, key: &str, typing: bool) -> Option<Action> {
        if typing && !matches!(key, "PageDown" | "PageUp") {
            return None;
        }
        self.bindings.iter().find(|(bound, _)| bound == key).map(|(_, action)| *action)
    }

    /// Assigns a key to an action (taking it away from any other action).
    pub fn bind(&mut self, key: &str, action: Action) {
        self.bindings.retain(|(bound, _)| bound != key);
        self.bindings.push((key.to_owned(), action));
    }

    /// The keys of one action, for display.
    pub fn keys_of(&self, action: Action) -> Vec<String> {
        self.bindings.iter().filter(|(_, bound)| *bound == action).map(|(key, _)| key.clone()).collect()
    }

    /// The mapping saved in this browser, or the default.
    pub fn load() -> KeyMap {
        storage()
            .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Saves the mapping in this browser (silently does nothing if storage is unavailable).
    pub fn save(&self) {
        if let (Some(storage), Ok(json)) = (storage(), serde_json::to_string(self)) {
            let _ = storage.set_item(STORAGE_KEY, &json);
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// A readable name for a key in the settings ("Spacja" instead of " ").
pub fn key_label(key: &str) -> String {
    match key {
        " " => "Spacja".to_owned(),
        "ArrowRight" => "→".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keys_turn_pages_and_black_out() {
        let map = KeyMap::default();
        assert_eq!(map.action_for("PageDown", false), Some(Action::Next));
        assert_eq!(map.action_for("ArrowLeft", false), Some(Action::Prev));
        assert_eq!(map.action_for("b", false), Some(Action::ToggleBlack));
        assert_eq!(map.action_for("x", false), None);
    }

    #[test]
    fn while_typing_only_page_keys_act() {
        let map = KeyMap::default();
        assert_eq!(map.action_for("PageDown", true), Some(Action::Next));
        assert_eq!(map.action_for("b", true), None, "typing a B must not black the screen");
        assert_eq!(map.action_for("ArrowRight", true), None);
    }

    #[test]
    fn learning_a_key_moves_it_to_the_new_action() {
        let mut map = KeyMap::default();
        map.bind("PageDown", Action::ToggleBlack);
        map.bind("F13", Action::Next);
        assert_eq!(map.action_for("PageDown", false), Some(Action::ToggleBlack));
        assert_eq!(map.action_for("F13", false), Some(Action::Next));
        assert!(map.keys_of(Action::Next).contains(&"F13".to_owned()));
        assert!(!map.keys_of(Action::Next).contains(&"PageDown".to_owned()));
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(serde_json::from_str::<KeyMap>(&json).unwrap(), map);
    }
}
