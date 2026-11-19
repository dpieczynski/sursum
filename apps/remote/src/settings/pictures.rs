// SPDX-License-Identifier: GPL-3.0-or-later
//! "Usuń nieużywane obrazy": the server counts the pictures nothing uses, the sheet says
//! how many and how much space, and only a hold removes them. Nothing is removed automatically.

use leptos::prelude::*;
use sursum_protocol::{PictureCleanup, PictureCleanupRequest};

use crate::api::post_json;
use crate::messages::{Msg, picture_cleanup_line, pl};
use crate::ui::{HoldButton, Sheet};

const URL: &str = "/api/assets/cleanup";

/// Where a clean-up is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Idle,
    Checking,
    /// Counted; waiting for the hold.
    Review(PictureCleanup),
    Removing,
    Removed(PictureCleanup),
    Failed,
}

#[component]
pub fn PictureCleanupButton() -> impl IntoView {
    let step = RwSignal::new(Step::Idle);
    let run = move |dry_run: bool| {
        step.set(if dry_run { Step::Checking } else { Step::Removing });
        leptos::task::spawn_local(async move {
            let answer: Option<PictureCleanup> = post_json(URL, &PictureCleanupRequest { dry_run }).await;
            step.set(match answer {
                None => Step::Failed,
                Some(cleanup) if dry_run => Step::Review(cleanup),
                Some(cleanup) => Step::Removed(cleanup),
            });
        });
    };
    let close = move || {
        if !matches!(step.get_untracked(), Step::Checking | Step::Removing) {
            step.set(Step::Idle);
        }
    };
    let reviewing = move || matches!(step.get(), Step::Review(cleanup) if cleanup.pictures > 0);
    let sheet = move || {
        (step.get() != Step::Idle).then(|| {
            let lines = move || {
                sheet_lines(step.get()).into_iter().map(|line| view! { <p>{line}</p> }).collect_view()
            };
            view! {
                <Sheet title=pl(Msg::PictureCleanupTitle).to_owned() on_close=close testid="picture-cleanup">
                    <div class="sheet-status" data-testid="picture-cleanup-answer">
                        {lines}
                    </div>
                    <Show when=reviewing>
                        <div class="sheet-actions">
                            <button class="btn" on:click=move |_| close()>
                                {pl(Msg::Cancel)}
                            </button>
                            <HoldButton
                                label=pl(Msg::PictureCleanupConfirm)
                                on_hold=move || run(false)
                                class="btn--danger"
                                testid="picture-cleanup-confirm"
                            />
                        </div>
                    </Show>
                </Sheet>
            }
        })
    };
    view! {
        <button class="btn btn--sm" data-testid="picture-cleanup" on:click=move |_| run(true)>
            {pl(Msg::PictureCleanup)}
        </button>
        {sheet}
    }
}

/// What the sheet says at each step.
fn sheet_lines(step: Step) -> Vec<String> {
    match step {
        Step::Idle => Vec::new(),
        Step::Checking | Step::Removing => vec![pl(Msg::PictureCleanupChecking).to_owned()],
        Step::Review(cleanup) if cleanup.pictures == 0 => vec![pl(Msg::PictureCleanupNone).to_owned()],
        Step::Review(cleanup) => {
            vec![picture_cleanup_line(&cleanup, false), pl(Msg::PictureCleanupKept).to_owned()]
        }
        Step::Removed(cleanup) => vec![picture_cleanup_line(&cleanup, true)],
        Step::Failed => vec![pl(Msg::PictureCleanupFailed).to_owned()],
    }
}
