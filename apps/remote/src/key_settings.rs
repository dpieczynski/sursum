// SPDX-License-Identifier: GPL-3.0-or-later
//! The pedal and key settings: which keys do what, and a learn mode ("press the pedal now").

use leptos::ev;
use leptos::prelude::*;

use crate::Commands;
use crate::keys::{Action, KeyMap, key_label};
use crate::messages::{Msg, pl};

/// The key mapping and the action waiting for a key in learn mode (shared through the context).
#[derive(Clone, Copy)]
pub struct Keys {
    pub map: RwSignal<KeyMap>,
    pub learning: RwSignal<Option<Action>>,
}

/// Listens to every key press on the page: in learn mode the key is assigned, otherwise its
/// action is sent. Held keys (auto-repeat) are ignored, so a pedal held down moves one slide.
pub fn listen(keys: Keys, commands: Commands) {
    let handle = window_event_listener(ev::keydown, move |event| {
        if event.repeat() {
            return;
        }
        let key = event.key();
        if let Some(action) = keys.learning.get_untracked() {
            event.prevent_default();
            keys.map.update(|map| map.bind(&key, action));
            keys.map.with_untracked(KeyMap::save);
            keys.learning.set(None);
            return;
        }
        let Some(action) = keys.map.with_untracked(|map| map.action_for(&key, typing_in_a_field(&event)))
        else {
            return;
        };
        event.prevent_default();
        commands.send(action.command());
    });
    // The listener lives as long as the page.
    std::mem::forget(handle);
}

fn typing_in_a_field(event: &web_sys::KeyboardEvent) -> bool {
    use wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|element| matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
}

/// The button that opens the key settings, and the settings while open.
#[component]
pub fn KeysButton() -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <button class="btn btn--sm" on:click=move |_| open.update(|open| *open = !*open)>
            {pl(Msg::Keys)}
        </button>
        <Show when=move || open.get()>
            <KeySettings open />
        </Show>
    }
}

/// The settings panel: one row per action with its keys and a "learn" button.
#[component]
pub fn KeySettings(open: RwSignal<bool>) -> impl IntoView {
    let keys = expect_context::<Keys>();
    let rows = Action::ALL.into_iter().map(|action| action_row(keys, action)).collect_view();
    let reset = move |_| {
        keys.map.set(KeyMap::default());
        keys.map.with_untracked(KeyMap::save);
    };
    let close = move |_| {
        keys.learning.set(None);
        open.set(false);
    };
    view! {
        <div class="key-settings" data-testid="key-settings">
            <p class="editor-help">{pl(Msg::KeysHelp)}</p>
            {rows}
            <div class="setting-row">
                <button type="button" class="btn btn--sm btn--quiet" on:click=reset>
                    {pl(Msg::KeysReset)}
                </button>
                <button type="button" class="btn btn--sm btn--primary" on:click=close>
                    {pl(Msg::Close)}
                </button>
            </div>
        </div>
    }
}

fn action_row(keys: Keys, action: Action) -> impl IntoView {
    let bound = move || {
        let names: Vec<String> =
            keys.map.with(|map| map.keys_of(action)).iter().map(|key| key_label(key)).collect();
        if names.is_empty() { pl(Msg::KeysNone).to_owned() } else { names.join(", ") }
    };
    let waiting = move || keys.learning.get() == Some(action);
    let learn = move |_| keys.learning.set(Some(action));
    let learn_label = move || if waiting() { pl(Msg::KeysPressNow) } else { pl(Msg::KeysLearn) };
    view! {
        <div class="key-row" class:selected=waiting data-action=action_name(action)>
            <span class="key-action">{pl(action_label(action))}</span>
            <span class="key-names">{bound}</span>
            <button type="button" class="btn btn--sm" on:click=learn>
                {learn_label}
            </button>
        </div>
    }
}

fn action_label(action: Action) -> Msg {
    match action {
        Action::Next => Msg::Next,
        Action::Prev => Msg::Prev,
        Action::NextItem => Msg::NextItem,
        Action::PrevItem => Msg::PrevItem,
        Action::ToggleBlack => Msg::Black,
    }
}

/// Stable names for test hooks (`data-action`).
fn action_name(action: Action) -> &'static str {
    match action {
        Action::Next => "next",
        Action::Prev => "prev",
        Action::NextItem => "next_item",
        Action::PrevItem => "prev_item",
        Action::ToggleBlack => "toggle_black",
    }
}
