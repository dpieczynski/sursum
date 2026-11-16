// SPDX-License-Identifier: GPL-3.0-or-later
//! Sharing the song library between churches: download a `.sursum` file, and merge one in.
//!
//! Merging always asks the server for a dry run first and shows what would change; nothing is
//! written until the operator confirms. The server does all comparing (ADR 0005 spirit: no
//! logic in the browser).

use leptos::prelude::*;
use sursum_protocol::MergeReport;
use wasm_bindgen::JsCast;

use crate::api::post_file;
use crate::messages::{Msg, merge_summary, pl};

/// Where the merge flow is.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Idle,
    Checking,
    /// The dry run finished; waiting for the operator to confirm.
    Review(MergeReport),
    Merging,
    Done(MergeReport),
    Failed,
}

/// Downloads the whole library (also a backup); any paired remote may do it.
#[component]
pub fn ExportLink() -> impl IntoView {
    view! {
        <a class="import-button" href="/api/library/export" download="biblioteka.sursum">
            {pl(Msg::ExportLibrary)}
        </a>
    }
}

/// "Merge library" picker with a dry run and a confirmation step (admin mode).
#[component]
pub fn MergeButton() -> impl IntoView {
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
        chosen.set_value(Some(file.clone()));
        step.set(Step::Checking);
        leptos::task::spawn_local(async move {
            let report = post_file::<MergeReport>("/api/library/merge?dry_run=true", &file).await;
            step.set(report.map_or(Step::Failed, Step::Review));
        });
    };
    let confirm = move |_| {
        let Some(file) = chosen.get_value() else { return };
        step.set(Step::Merging);
        leptos::task::spawn_local(async move {
            let report = post_file::<MergeReport>("/api/library/merge", &file).await;
            step.set(report.map_or(Step::Failed, Step::Done));
        });
    };
    let cancel = move |_| {
        chosen.set_value(None);
        step.set(Step::Idle);
    };
    let reviewing = move || matches!(step.get(), Step::Review(_));
    view! {
        <label class="import-button">
            {pl(Msg::MergeLibrary)} <input type="file" accept=".sursum" on:change=on_change />
        </label>
        <span class="import-status" data-testid="merge-status">
            {move || status_text(&step.get())}
        </span>
        <Show when=reviewing>
            <button class="import-button" data-testid="merge-confirm" on:click=confirm>
                {pl(Msg::MergeConfirm)}
            </button>
            <button class="import-button" on:click=cancel>
                {pl(Msg::Cancel)}
            </button>
        </Show>
    }
}

fn status_text(step: &Step) -> String {
    match step {
        Step::Idle => String::new(),
        Step::Checking => pl(Msg::MergeChecking).to_owned(),
        Step::Review(report) => format!("{} {}", pl(Msg::MergeWouldDo), summary(report)),
        Step::Merging => pl(Msg::Merging).to_owned(),
        Step::Done(report) => format!("{} {}", pl(Msg::MergeDone), summary(report)),
        Step::Failed => pl(Msg::MergeFailed).to_owned(),
    }
}

fn summary(report: &MergeReport) -> String {
    merge_summary(report.added, report.updated, report.unchanged, report.kept_local, &report.conflicts)
}
