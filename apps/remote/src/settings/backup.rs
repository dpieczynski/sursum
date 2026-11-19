// SPDX-License-Identifier: GPL-3.0-or-later
//! Dane urządzenia (ADR 0018): downloading a whole-device backup (any paired remote) and
//! restoring one. The server checks the file, answers what it holds, and restarts with
//! it; the remote reloads once the sheet is closed, so nothing it shows is from before. Removing
//! unused pictures lives here too ([`super::pictures`]).

use leptos::prelude::*;
use sursum_protocol::RestoreOutcome;
use wasm_bindgen::JsCast;

use super::Section;
use crate::api::post_file;
use crate::messages::{Msg, backup_summary_lines, pl, restore_refusal};
use crate::ui::Sheet;

/// Where a restore is.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Idle,
    /// A file was chosen; its name is shown with the warning.
    Confirm(String),
    Restoring,
    /// The server's answer, or `None` when the upload did not get through.
    Done(Option<RestoreOutcome>),
}

#[component]
pub fn BackupSection() -> impl IntoView {
    view! {
        <Section title=Msg::SectionBackup>
            <p class="hint">{pl(Msg::BackupHint)}</p>
            <div class="setting-row">
                <a class="btn btn--sm" href="/api/backup" download=file_name() data-testid="backup-download">
                    {pl(Msg::BackupDownload)}
                </a>
            </div>
            <div class="setting-row">
                <RestoreButton />
            </div>
            <div class="setting-row">
                <super::pictures::PictureCleanupButton />
            </div>
        </Section>
    }
}

/// `kopia-sursum-2026-11-17.sursum-backup`, dated by the browser's clock.
fn file_name() -> String {
    let now = js_sys::Date::new_0();
    format!(
        "kopia-sursum-{}-{:02}-{:02}.sursum-backup",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}

/// A file picker; the chosen backup is restored only after the warning was confirmed.
#[component]
fn RestoreButton() -> impl IntoView {
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
    let restore = move |_| {
        let Some(file) = chosen.get_value() else { return };
        step.set(Step::Restoring);
        leptos::task::spawn_local(async move {
            step.set(Step::Done(post_file::<RestoreOutcome>("/api/backup/restore", &file).await));
        });
    };
    let close = move || match step.get_untracked() {
        Step::Restoring => {}
        Step::Done(Some(RestoreOutcome::Restored { .. })) => crate::reload(),
        _ => {
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
                <Sheet title=pl(Msg::BackupRestoreTitle).to_owned() on_close=close testid="restore">
                    <div class="sheet-status" data-testid="restore-answer">
                        {lines}
                    </div>
                    <Show when=confirming>
                        <div class="sheet-actions">
                            <button class="btn" on:click=move |_| close()>
                                {pl(Msg::Cancel)}
                            </button>
                            <button class="btn btn--danger" data-testid="restore-confirm" on:click=restore>
                                {pl(Msg::BackupConfirm)}
                            </button>
                        </div>
                    </Show>
                </Sheet>
            }
        })
    };
    view! {
        <label class="btn btn--sm">
            {pl(Msg::BackupRestore)} <input type="file" accept=".sursum-backup" on:change=on_change />
        </label>
        {sheet}
    }
}

/// What the sheet says at each step.
fn sheet_lines(step: &Step) -> Vec<String> {
    match step {
        Step::Idle => Vec::new(),
        Step::Confirm(name) => vec![name.clone(), pl(Msg::BackupWarning).to_owned()],
        Step::Restoring => vec![pl(Msg::BackupRestoring).to_owned()],
        Step::Done(Some(RestoreOutcome::Restored { summary })) => {
            let mut lines = backup_summary_lines(summary, local_date(summary.created_unix)).to_vec();
            lines.push(pl(Msg::BackupRestored).to_owned());
            lines
        }
        Step::Done(Some(RestoreOutcome::Refused { reason })) => vec![pl(restore_refusal(*reason)).to_owned()],
        Step::Done(None) => vec![pl(Msg::UpdateSendFailed).to_owned()],
    }
}

/// Day, month, year, hour and minute of a Unix time in the browser's time zone.
fn local_date(unix_seconds: u64) -> [u32; 5] {
    #[allow(clippy::cast_precision_loss, reason = "seconds since 1970 fit an f64 exactly")]
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(unix_seconds as f64 * 1000.0));
    [date.get_date(), date.get_month() + 1, date.get_full_year(), date.get_hours(), date.get_minutes()]
}
