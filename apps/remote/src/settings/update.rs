// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates in "O programie" on devices running the sursum image: one signed `.sursum-system` file
//! brings the server, the kiosk and the system together (ADR 0019, ADR 0022). The server checks
//! it before anything changes; the new version runs on trial and the device goes back by itself
//! if it does not come up. The remote shows how the last update ended and reconnects (and
//! reloads) by itself after the restart.

use leptos::prelude::*;
use sursum_protocol::{InstallOutcome, UpdateStatus};
use wasm_bindgen::JsCast;

use crate::api::{get_json, post_file};
use crate::maintenance::Maintenance;
use crate::messages::{Msg, install_refusal, installed_version, pl, update_lines};
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

/// The update state and, where this device can take them, the install buttons.
#[component]
pub fn Updates() -> impl IntoView {
    let status = RwSignal::new(None::<UpdateStatus>);
    leptos::task::spawn_local(async move { status.set(get_json("/api/update/status").await) });
    let lines = move || {
        status.get().map(|status| {
            update_lines(&status)
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
    let can_install =
        move || status.get().and_then(|status| status.system).is_some_and(|system| system.can_install);
    view! {
        {lines}
        <Show when=can_install>
            <div class="setting-row">
                <InstallButton />
            </div>
        </Show>
    }
}

/// A file picker; the chosen file is installed only after the warning was confirmed.
#[component]
fn InstallButton() -> impl IntoView {
    let maintenance = expect_context::<Maintenance>();
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
            match post_file::<InstallOutcome>("/api/system/update", &file).await {
                // The maintenance sheet follows it from here, over every view.
                Some(InstallOutcome::Installed { version }) => {
                    chosen.set_value(None);
                    step.set(Step::Idle);
                    maintenance.start(version);
                }
                outcome => step.set(Step::Done(outcome)),
            }
        });
    };
    // Nothing else should look usable while an upload runs or the device updates its system.
    let locked = move || step.get() == Step::Installing;
    let close = move || {
        if !locked() {
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
                <Sheet
                    title=pl(Msg::SystemUpdateTitle).to_owned()
                    on_close=close
                    testid="system-update"
                    locked=Signal::derive(locked)
                >
                    <Show when=locked>
                        <div class="progress-spinner" role="progressbar" aria-busy="true"></div>
                    </Show>
                    <div class="sheet-status" data-testid="system-update-answer">
                        {lines}
                    </div>
                    <Show when=confirming>
                        <div class="sheet-actions">
                            <button class="btn" on:click=move |_| close()>
                                {pl(Msg::Cancel)}
                            </button>
                            <button
                                class="btn btn--primary"
                                data-testid="system-update-confirm"
                                on:click=install
                            >
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
            {pl(Msg::UpdateInstall)} <input type="file" accept=".sursum-system" on:change=on_change />
        </label>
        {sheet}
    }
}

/// What the sheet says at each step.
fn sheet_lines(step: &Step) -> Vec<String> {
    match step {
        Step::Idle => Vec::new(),
        Step::Confirm(name) => vec![name.clone(), pl(Msg::SystemUpdateWarning).to_owned()],
        Step::Installing => vec![pl(Msg::UpdateInstalling).to_owned()],
        Step::Done(Some(InstallOutcome::Installed { version })) => {
            vec![installed_version(version), pl(Msg::SystemUpdateInstalled).to_owned()]
        }
        Step::Done(Some(InstallOutcome::Refused { reason })) => vec![pl(install_refusal(*reason)).to_owned()],
        Step::Done(None) => vec![pl(Msg::UpdateSendFailed).to_owned()],
    }
}
