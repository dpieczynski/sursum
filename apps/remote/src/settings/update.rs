// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates in "O programie" (admin): the program (`.sursum-app`, ADR 0013, second amendment) and,
//! on devices running the sursum image, the system (`.sursum-system`, ADR 0019). Both are signed
//! files the server checks before anything changes; both end on trial and are undone by the
//! device itself if the new version does not come up. The remote shows how the last ones ended
//! and reconnects (or reloads) by itself after the restart.

use leptos::prelude::*;
use sursum_protocol::{InstallOutcome, UpdateStatus};
use wasm_bindgen::JsCast;

use super::AdminOnly;
use crate::api::{get_json, post_file};
use crate::messages::{Msg, install_refusal, installed_version, last_update, pl, system_lines};
use crate::ui::Sheet;

/// What differs between a program and a system update.
#[derive(Debug, Clone, Copy)]
struct Kind {
    url: &'static str,
    accept: &'static str,
    label: Msg,
    title: Msg,
    warning: Msg,
    installed: Msg,
    testid: &'static str,
}

const PROGRAM: Kind = Kind {
    url: "/api/update",
    accept: ".sursum-app",
    label: Msg::UpdateInstall,
    title: Msg::UpdateTitle,
    warning: Msg::UpdateWarning,
    installed: Msg::UpdateInstalled,
    testid: "update",
};

const SYSTEM: Kind = Kind {
    url: "/api/system/update",
    accept: ".sursum-system",
    label: Msg::SystemUpdateInstall,
    title: Msg::SystemUpdateTitle,
    warning: Msg::SystemUpdateWarning,
    installed: Msg::SystemUpdateInstalled,
    testid: "system-update",
};

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
    let can_install_system =
        move || status.get().and_then(|status| status.system).is_some_and(|system| system.can_install);
    view! {
        {lines}
        <Show when=can_install>
            <div class="setting-row">
                <AdminOnly label=PROGRAM.label>
                    <InstallButton kind=PROGRAM />
                </AdminOnly>
            </div>
        </Show>
        <Show when=can_install_system>
            <div class="setting-row">
                <AdminOnly label=SYSTEM.label>
                    <InstallButton kind=SYSTEM />
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
    if let Some(system) = &status.system {
        lines.extend(system_lines(system));
    }
    lines
}

/// A file picker; the chosen file is installed only after the warning was confirmed.
#[component]
fn InstallButton(kind: Kind) -> impl IntoView {
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
            step.set(Step::Done(post_file::<InstallOutcome>(kind.url, &file).await));
        });
    };
    let close = move || {
        if step.get_untracked() != Step::Installing {
            chosen.set_value(None);
            step.set(Step::Idle);
        }
    };
    let confirming = move || matches!(step.get(), Step::Confirm(_));
    let answer_id = format!("{}-answer", kind.testid);
    let confirm_id = format!("{}-confirm", kind.testid);
    let sheet = move || {
        (step.get() != Step::Idle).then(|| {
            let lines = move || {
                sheet_lines(kind, &step.get()).into_iter().map(|line| view! { <p>{line}</p> }).collect_view()
            };
            let (answer_id, confirm_id) = (answer_id.clone(), confirm_id.clone());
            view! {
                <Sheet title=pl(kind.title).to_owned() on_close=close testid=kind.testid>
                    <div class="sheet-status" data-testid=answer_id>
                        {lines}
                    </div>
                    <Show when=confirming>
                        <div class="sheet-actions">
                            <button class="btn" on:click=move |_| close()>
                                {pl(Msg::Cancel)}
                            </button>
                            <button class="btn btn--primary" data-testid=confirm_id.clone() on:click=install>
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
            {pl(kind.label)} <input type="file" accept=kind.accept on:change=on_change />
        </label>
        {sheet}
    }
}

/// What the sheet says at each step.
fn sheet_lines(kind: Kind, step: &Step) -> Vec<String> {
    match step {
        Step::Idle => Vec::new(),
        Step::Confirm(name) => vec![name.clone(), pl(kind.warning).to_owned()],
        Step::Installing => vec![pl(Msg::UpdateInstalling).to_owned()],
        Step::Done(Some(InstallOutcome::Installed { version })) => {
            vec![installed_version(version), pl(kind.installed).to_owned()]
        }
        Step::Done(Some(InstallOutcome::Refused { reason })) => vec![pl(install_refusal(*reason)).to_owned()],
        Step::Done(None) => vec![pl(Msg::UpdateSendFailed).to_owned()],
    }
}
