// SPDX-License-Identifier: GPL-3.0-or-later
//! Program updates in "O programie": how the last update ended, and installing a signed
//! `.sursum-app` file (admin). The server checks the signature, answers, and restarts into the
//! new version; the remote then reloads by itself (new build id). A version that does not work
//! is undone by the device itself (ADR 0013, second amendment).

use leptos::prelude::*;
use sursum_protocol::{InstallOutcome, UpdateStatus};
use wasm_bindgen::JsCast;

use super::AdminOnly;
use crate::api::{get_json, post_file};
use crate::messages::{Msg, install_refusal, installed_version, last_update, pl};
use crate::ui::Sheet;

/// Where an install is.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Idle,
    /// A file was chosen; its name is shown with the warning.
    Confirm(String),
    Installing,
    /// The server's answer, or `None` when the upload did not get through.
    Done(Option<InstallOutcome>),
}

/// The update state and, where this device can take updates, the install button.
#[component]
pub fn Updates() -> impl IntoView {
    let status = RwSignal::new(None::<UpdateStatus>);
    leptos::task::spawn_local(async move { status.set(get_json("/api/update/status").await) });
    let lines = move || {
        status.get().map(|status| {
            status_lines(&status)
                .into_iter()
                .map(|line| {
                    view! {
                        <p class="hint" data-testid="update-status">
                            {line}
                        </p>
                    }
                })
                .collect_view()
        })
    };
    let can_install = move || status.get().is_some_and(|status| status.can_install);
    view! {
        {lines}
        <Show when=can_install>
            <div class="setting-row">
                <AdminOnly label=Msg::UpdateInstall>
                    <InstallButton />
                </AdminOnly>
            </div>
        </Show>
    }
}

fn status_lines(status: &UpdateStatus) -> Vec<String> {
    let mut lines = Vec::new();
    if status.on_trial {
        lines.push(pl(Msg::UpdateOnTrial).to_owned());
    }
    if let Some(last) = &status.last {
        lines.push(last_update(last));
    }
    lines
}

/// A file picker; the chosen file is installed only after the warning was confirmed.
#[component]
fn InstallButton() -> impl IntoView {
    let step = RwSignal::new(Step::Idle);
    // `web_sys::File` is not `Send`, so it lives in a local store.
    let chosen = StoredValue::new_local(None::<web_sys::File>);
    let on_change = move |event: leptos::ev::Event| {
        let Some(input) =
            event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
        input.set_value("");
        step.set(Step::Confirm(file.name()));
        chosen.set_value(Some(file));
    };
    let install = move |_| {
        let Some(file) = chosen.get_value() else { return };
        step.set(Step::Installing);
        leptos::task::spawn_local(async move {
            step.set(Step::Done(post_file::<InstallOutcome>("/api/update", &file).await));
        });
    };
    let close = move || {
        if step.get_untracked() != Step::Installing {
            chosen.set_value(None);
            step.set(Step::Idle);
        }
    };
    let confirming = move || matches!(step.get(), Step::Confirm(_));
    let sheet = move || {
        (step.get() != Step::Idle).then(|| {
            let lines = move || {
                sheet_lines(&step.get()).into_iter().map(|line| view! { <p>{line}</p> }).collect_view()
            };
            view! {
                <Sheet title=pl(Msg::UpdateTitle).to_owned() on_close=close testid="update">
                    <div class="sheet-status" data-testid="update-answer">
                        {lines}
                    </div>
                    <Show when=confirming>
                        <div class="sheet-actions">
                            <button class="btn" on:click=move |_| close()>
                                {pl(Msg::Cancel)}
                            </button>
                            <button class="btn btn--primary" data-testid="update-confirm" on:click=install>
                                {pl(Msg::UpdateConfirm)}
                            </button>
                        </div>
                    </Show>
                </Sheet>
            }
        })
    };
    view! {
        <label class="btn btn--sm">
            {pl(Msg::UpdateInstall)} <input type="file" accept=".sursum-app" on:change=on_change />
        </label>
        {sheet}
    }
}

/// What the sheet says at each step.
fn sheet_lines(step: &Step) -> Vec<String> {
    match step {
        Step::Idle => Vec::new(),
        Step::Confirm(name) => vec![name.clone(), pl(Msg::UpdateWarning).to_owned()],
        Step::Installing => vec![pl(Msg::UpdateInstalling).to_owned()],
        Step::Done(Some(InstallOutcome::Installed { version })) => {
            vec![installed_version(version), pl(Msg::UpdateInstalled).to_owned()]
        }
        Step::Done(Some(InstallOutcome::Refused { reason })) => vec![pl(install_refusal(*reason)).to_owned()],
        Step::Done(None) => vec![pl(Msg::UpdateSendFailed).to_owned()],
    }
}
