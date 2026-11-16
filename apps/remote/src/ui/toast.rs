// SPDX-License-Identifier: GPL-3.0-or-later
//! The toast: one short message above the controls that goes away by itself. It is laid over
//! the page, so a message never moves a button under the operator's finger. A toast may offer
//! one command to take the last action back ("Cofnij").

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use sursum_protocol::Command;

use crate::messages::{Msg, pl};

/// How long a plain message stays.
const SHORT_MS: u32 = 4_000;
/// How long a message with "undo" stays.
const UNDO_MS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq)]
struct Shown {
    text: String,
    undo: Option<Command>,
}

/// The toast queue (shared through the context). A new message replaces the old one.
#[derive(Clone, Copy)]
pub struct Toasts {
    shown: RwSignal<Option<Shown>>,
    generation: StoredValue<u32>,
}

impl Default for Toasts {
    fn default() -> Self {
        Toasts { shown: RwSignal::new(None), generation: StoredValue::new(0) }
    }
}

impl Toasts {
    /// Shows a message for a few seconds.
    pub fn show(self, text: impl Into<String>) {
        self.put(Shown { text: text.into(), undo: None }, SHORT_MS);
    }

    /// Shows a message with an "undo" button that sends `undo`.
    pub fn show_with_undo(self, text: impl Into<String>, undo: Command) {
        self.put(Shown { text: text.into(), undo: Some(undo) }, UNDO_MS);
    }

    /// Hides the message now.
    pub fn hide(self) {
        self.shown.set(None);
    }

    fn put(self, shown: Shown, duration_ms: u32) {
        self.generation.update_value(|generation| *generation = generation.wrapping_add(1));
        let this = self.generation.get_value();
        self.shown.set(Some(shown));
        leptos::task::spawn_local(async move {
            TimeoutFuture::new(duration_ms).await;
            if self.generation.get_value() == this {
                self.shown.set(None);
            }
        });
    }
}

/// Where the toast is drawn; `send` runs the undo command.
#[component]
pub fn Toast(toasts: Toasts, send: impl Fn(Command) + Copy + Send + Sync + 'static) -> impl IntoView {
    let content = move || {
        toasts.shown.get().map(|shown| {
            let undo = shown.undo.map(|command| {
                let run = move |_| {
                    send(command.clone());
                    toasts.hide();
                };
                view! {
                    <button class="btn btn--sm btn--primary" on:click=run>
                        {pl(Msg::UndoShort)}
                    </button>
                }
            });
            view! {
                <div class="toast" role="status" data-testid="toast">
                    <span class="toast-text">{shown.text}</span>
                    {undo}
                </div>
            }
        })
    };
    view! { {content} }
}
