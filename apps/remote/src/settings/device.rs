// SPDX-License-Identifier: GPL-3.0-or-later
//! To urządzenie: settings of this tablet or PC (kept in its browser): which side the controls
//! are on, the pedal keys, vibration, and full screen.

use leptos::prelude::*;

use super::Section;
use crate::messages::{Msg, pl};
use crate::prefs::{Hand, Prefs};

#[component]
pub fn DeviceSection() -> impl IntoView {
    let prefs = expect_context::<Prefs>();
    let side = move |hand: Hand, label: Msg, testid: &'static str| {
        view! {
            <label class="choice">
                <input
                    type="radio"
                    name="hand"
                    data-testid=testid
                    prop:checked=move || prefs.hand.get() == hand
                    on:change=move |_| prefs.set_hand(hand)
                />
                {pl(label)}
            </label>
        }
    };
    let full = RwSignal::new(is_fullscreen());
    let toggle_full = move |_| {
        toggle_fullscreen();
        // The change is asynchronous; read it back a moment later.
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(300).await;
            full.set(is_fullscreen());
        });
    };
    view! {
        <Section title=Msg::SectionDevice>
            <fieldset class="choices">
                <legend>{pl(Msg::ButtonsSide)}</legend>
                {side(Hand::Right, Msg::ButtonsRight, "hand-right")}
                {side(Hand::Left, Msg::ButtonsLeft, "hand-left")}
            </fieldset>
            <Show when=crate::prefs::can_vibrate>
                <label class="choice">
                    <input
                        type="checkbox"
                        prop:checked=move || prefs.vibration.get()
                        on:change=move |event| prefs.set_vibration(event_target_checked(&event))
                    />
                    {pl(Msg::Vibration)}
                </label>
            </Show>
            <div class="setting-row">
                <button class="btn btn--sm" on:click=toggle_full>
                    {move || if full.get() { pl(Msg::ExitFullscreen) } else { pl(Msg::Fullscreen) }}
                </button>
                <crate::key_settings::KeysButton />
            </div>
            <p class="hint">{pl(Msg::SleepTip)}</p>
        </Section>
    }
}

fn is_fullscreen() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.fullscreen_element())
        .is_some()
}

/// Full screen hides the browser's bars (it works without HTTPS).
fn toggle_fullscreen() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
    } else if let Some(root) = document.document_element() {
        let _ = root.request_fullscreen();
    }
}
