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
use crate::messages::{Msg, merge_lines, pl};
use crate::ui::Sheet;

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
        <a class="btn btn--sm" href="/api/library/export" download="biblioteka.sursum">
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
    let close = move || {
        if !matches!(step.get_untracked(), Step::Checking | Step::Merging) {
            chosen.set_value(None);
            step.set(Step::Idle);
        }
    };
    let sheet = move || {
        (step.get() != Step::Idle).then(|| {
            view! {
                <Sheet title=pl(Msg::MergeLibrary).to_owned() on_close=close testid="merge">
                    <p class="sheet-status" data-testid="merge-status">
                        {move || heading(&step.get())}
                    </p>
                    <ul class="report-lines">{move || report_lines(step.get())}</ul>
                    <Show when=reviewing>
                        <div class="sheet-actions">
                            <button class="btn" on:click=cancel>
                                {pl(Msg::Cancel)}
                            </button>
                            <button class="btn btn--primary" data-testid="merge-confirm" on:click=confirm>
                                {pl(Msg::MergeConfirm)}
                            </button>
                        </div>
                    </Show>
                </Sheet>
            }
        })
    };
    view! {
        <label class="btn btn--sm">
            {pl(Msg::MergeLibrary)} <input type="file" accept=".sursum" on:change=on_change />
        </label>
        {sheet}
    }
}

/// What the sheet says at each step.
fn heading(step: &Step) -> &'static str {
    pl(match step {
        Step::Idle | Step::Checking => Msg::MergeChecking,
        Step::Review(_) => Msg::MergeWouldDo,
        Step::Merging => Msg::Merging,
        Step::Done(_) => Msg::MergeDone,
        Step::Failed => Msg::MergeFailed,
    })
}

/// The report as a list: one line per number, conflicts by title.
fn report_lines(step: Step) -> impl IntoView {
    let lines = match &step {
        Step::Review(report) | Step::Done(report) => merge_lines(report),
        _ => Vec::new(),
    };
    lines.into_iter().map(|line| view! { <li>{line}</li> }).collect_view()
}
