// SPDX-License-Identifier: GPL-3.0-or-later
//! The pedal and key settings: which keys do what, and a learn mode ("press the pedal now").

use leptos::ev;
use leptos::prelude::*;

use crate::Commands;
use crate::keys::{Action, Focus, KeyMap, key_label};
use crate::messages::{Msg, pl};

/// The key mapping and the action waiting for a key in learn mode (shared through the context).
#[derive(Clone, Copy)]
pub struct Keys {
    pub map: RwSignal<KeyMap>,
    pub learning: RwSignal<Option<Action>>,
}

/// Listens to every key press on the page: in learn mode the key is assigned (Escape cancels),
/// otherwise its action is sent, if this installation has a screen. Held keys (auto-repeat) and
/// shortcuts with Ctrl, Alt or Meta (the browser's and the editor's) are left alone, so a pedal
/// held down moves one slide and Alt+← still goes back.
pub fn listen(keys: Keys, commands: Commands, installation: sursum_protocol::Installation) {
    let handle = window_event_listener(ev::keydown, move |event| {
        if event.repeat() || event.ctrl_key() || event.alt_key() || event.meta_key() {
            return;
        }
        let key = event.key();
        if let Some(action) = keys.learning.get_untracked() {
            event.prevent_default();
            if key != "Escape" {
                keys.map.update(|map| map.bind(&key, action));
                keys.map.with_untracked(KeyMap::save);
            }
            keys.learning.set(None);
            return;
        }
        if installation == sursum_protocol::Installation::Library {
            return;
        }
        let Some(action) = keys.map.with_untracked(|map| map.action_for(&key, focus_of(&event))) else {
            return;
        };
        event.prevent_default();
        commands.send(action.command());
    });
    // The listener lives as long as the page.
    std::mem::forget(handle);
}

fn focus_of(event: &web_sys::KeyboardEvent) -> Focus {
    use wasm_bindgen::JsCast;
    let Some(element) = event.target().and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return Focus::Page;
    };
    match element.tag_name().as_str() {
        "INPUT" | "TEXTAREA" | "SELECT" => Focus::Field,
        _ if element.is_content_editable() => Focus::Field,
        "BUTTON" | "A" | "SUMMARY" => Focus::Control,
        _ => Focus::Page,
    }
}

/// The button that opens the key settings, and the settings while open.
#[component]
pub fn KeysButton() -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <button
            class="btn btn--sm disclosure"
            aria-expanded=move || open.get().to_string()
            on:click=move |_| open.update(|open| *open = !*open)
        >
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
    // However the panel closes (also through the button that opened it), learn mode ends with
    // it: a pending learn would otherwise swallow the next pedal press during the service.
    on_cleanup(move || keys.learning.set(None));
    let close = move |_| open.set(false);
    // The panel opens below the button, often below the fold: bring it into view, or the
    // button seems to do nothing (and a second tap closes it again).
    let panel = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        let Some(panel) = panel.get() else { return };
        request_animation_frame(move || {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            options.set_behavior(web_sys::ScrollBehavior::Smooth);
            panel.scroll_into_view_with_scroll_into_view_options(&options);
        });
    });
    view! {
        <div class="key-settings" data-testid="key-settings" node_ref=panel>
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
