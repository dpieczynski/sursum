// SPDX-License-Identifier: GPL-3.0-or-later
//! Importing an OpenSong library (songs and sets) from a ZIP file chosen on the tablet or PC.

use leptos::prelude::*;
use sursum_protocol::ImportReport;
use wasm_bindgen::JsCast;

use crate::api::post_file;
use crate::messages::{Msg, import_summary, pl};

/// A file picker for a ZIP of the OpenSong folder; shows a one-line report afterwards.
#[component]
pub fn ImportButton() -> impl IntoView {
    let status = RwSignal::new(None::<String>);
    let on_change = move |event: leptos::ev::Event| {
        let Some(input) =
            event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
        input.set_value("");
        status.set(Some(pl(Msg::Importing).to_owned()));
        leptos::task::spawn_local(async move {
            let report = post_file::<ImportReport>("/api/import/opensong", &file).await;
            status.set(Some(report.as_ref().map_or_else(|| pl(Msg::ImportFailed).to_owned(), summary)));
        });
    };
    view! {
        <label class="import-button">
            {pl(Msg::ImportOpenSong)} <input type="file" accept=".zip,application/zip" on:change=on_change />
        </label>
        <span class="import-status">{move || status.get()}</span>
    }
}

fn summary(report: &ImportReport) -> String {
    import_summary(
        report.inserted,
        report.updated,
        report.unchanged,
        report.failed,
        report.sets,
        report.problems.len(),
    )
}
