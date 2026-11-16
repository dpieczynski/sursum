// SPDX-License-Identifier: GPL-3.0-or-later
//! Importing an OpenSong library (songs and sets) from a ZIP file chosen on the tablet or PC.

use leptos::prelude::*;
use sursum_protocol::ImportReport;
use wasm_bindgen::JsCast;

use crate::api::post_file;
use crate::messages::{Msg, import_lines, pl};
use crate::ui::Sheet;

/// Where the import is.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Idle,
    Importing,
    Done(ImportReport),
    Failed,
}

/// A file picker for a ZIP of the OpenSong folder; the report opens in a sheet afterwards.
#[component]
pub fn ImportButton() -> impl IntoView {
    let step = RwSignal::new(Step::Idle);
    let on_change = move |event: leptos::ev::Event| {
        let Some(input) =
            event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
        input.set_value("");
        step.set(Step::Importing);
        leptos::task::spawn_local(async move {
            let report = post_file::<ImportReport>("/api/import/opensong", &file).await;
            step.set(report.map_or(Step::Failed, Step::Done));
        });
    };
    let close = move || {
        if step.get_untracked() != Step::Importing {
            step.set(Step::Idle);
        }
    };
    let sheet = move || {
        (step.get() != Step::Idle).then(|| {
            let (heading, lines) = match step.get() {
                Step::Importing => (pl(Msg::Importing), Vec::new()),
                Step::Done(report) => (pl(Msg::ImportDone), import_lines(&report)),
                Step::Failed | Step::Idle => (pl(Msg::ImportFailed), Vec::new()),
            };
            view! {
                <Sheet title=pl(Msg::ImportOpenSong).to_owned() on_close=close testid="import">
                    <p class="sheet-status" data-testid="import-status">
                        {heading}
                    </p>
                    <ul class="report-lines">
                        {lines.into_iter().map(|line| view! { <li>{line}</li> }).collect_view()}
                    </ul>
                </Sheet>
            }
        })
    };
    view! {
        <label class="btn btn--sm">
            {pl(Msg::ImportOpenSong)} <input type="file" accept=".zip,application/zip" on:change=on_change />
        </label>
        {sheet}
    }
}
