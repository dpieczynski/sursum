// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates in "O programie" (admin): the program (`.sursum-app`, ADR 0013, second amendment) and,
//! on devices running the sursum image, the system (`.sursum-system`, ADR 0019). Both are signed
//! files the server checks before anything changes; both end on trial and are undone by the
//! device itself if the new version does not come up. The remote shows how the last ones ended
//! and reconnects (or reloads) by itself after the restart.

use leptos::prelude::*;
use sursum_protocol::{InstallOutcome, LastSystemUpdate, UpdateStatus};
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
    /// After the upload, follow the device through writing, restarting and the trial (system
    /// updates), with the sheet locked until it ends.
    follow: bool,
}

const PROGRAM: Kind = Kind {
    url: "/api/update",
    accept: ".sursum-app",
    label: Msg::UpdateInstall,
    title: Msg::UpdateTitle,
    warning: Msg::UpdateWarning,
    installed: Msg::UpdateInstalled,
    testid: "update",
    follow: false,
};

const SYSTEM: Kind = Kind {
    url: "/api/system/update",
    accept: ".sursum-system",
    label: Msg::SystemUpdateInstall,
    title: Msg::SystemUpdateTitle,
    warning: Msg::SystemUpdateWarning,
    installed: Msg::SystemUpdateInstalled,
    testid: "system-update",
    follow: true,
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
    /// A system update was accepted; the device is somewhere between writing it and its trial.
    Following(String, Phase),
}

/// Where an accepted system update is, as far as the remote can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Writing,
    /// No answer from the server: the device is restarting.
    Restarting,
    Checking,
    Committed,
    RolledBack,
}

impl Phase {
    fn finished(self) -> bool {
        matches!(self, Phase::Committed | Phase::RolledBack)
    }
}

/// The phase of the system update `version` from a status answer (`None`: no answer).
fn phase_of(status: Option<&UpdateStatus>, version: &str) -> Phase {
    let Some(system) = status.and_then(|status| status.system.as_ref()) else { return Phase::Restarting };
    if system.installing {
        return Phase::Writing;
    }
    if system.on_trial {
        return Phase::Checking;
    }
    match &system.last {
        Some(LastSystemUpdate::Committed { version: done }) if done == version => Phase::Committed,
        Some(LastSystemUpdate::RolledBack { failed }) if failed == version => Phase::RolledBack,
        // Between the restart and the trial check, or an older outcome: keep waiting.
        _ => Phase::Restarting,
    }
}

/// Asks for the state every 3 s until the update `version` has ended.
async fn follow(step: RwSignal<Step>, version: String) {
    loop {
        gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
        let status: Option<UpdateStatus> = get_json("/api/update/status").await;
        let phase = phase_of(status.as_ref(), &version);
        step.set(Step::Following(version.clone(), phase));
        if phase.finished() {
            return;
        }
    }
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
            let outcome = post_file::<InstallOutcome>(kind.url, &file).await;
            match outcome {
                Some(InstallOutcome::Installed { version }) if kind.follow => {
                    step.set(Step::Following(version.clone(), Phase::Writing));
                    follow(step, version).await;
                }
                outcome => step.set(Step::Done(outcome)),
            }
        });
    };
    // Nothing else should look usable while an upload runs or the device updates its system.
    let locked = move || match step.get() {
        Step::Installing => true,
        Step::Following(_, phase) => !phase.finished(),
        _ => false,
    };
    let close = move || {
        if !locked() {
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
                <Sheet
                    title=pl(kind.title).to_owned()
                    on_close=close
                    testid=kind.testid
                    locked=Signal::derive(locked)
                >
                    <Show when=locked>
                        <div class="progress-spinner" role="progressbar" aria-busy="true"></div>
                    </Show>
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
        Step::Following(version, phase) => {
            let now = match phase {
                Phase::Writing => Msg::SystemUpdateWriting,
                Phase::Restarting => Msg::SystemUpdateRestarting,
                Phase::Checking => Msg::SystemUpdateChecking,
                Phase::Committed => Msg::SystemUpdateCommitted,
                Phase::RolledBack => Msg::SystemUpdateRolledBack,
            };
            let mut lines = vec![installed_version(version), pl(now).to_owned()];
            if !phase.finished() {
                lines.push(pl(Msg::SystemUpdateDontSwitchOff).to_owned());
            }
            lines
        }
    }
}

#[cfg(test)]
mod tests {
    use sursum_protocol::SystemStatus;

    use super::*;

    fn status(installing: bool, on_trial: bool, last: Option<LastSystemUpdate>) -> UpdateStatus {
        UpdateStatus {
            build: "b".into(),
            release: None,
            on_trial: false,
            last: None,
            can_install: true,
            system: Some(SystemStatus {
                version: "v1".into(),
                on_trial,
                installing,
                last,
                can_install: false,
            }),
        }
    }

    #[test]
    fn a_system_update_is_followed_from_writing_to_its_end() {
        assert_eq!(phase_of(Some(&status(true, false, None)), "v2"), Phase::Writing);
        assert_eq!(phase_of(None, "v2"), Phase::Restarting);
        assert_eq!(phase_of(Some(&status(false, true, None)), "v2"), Phase::Checking);
        let older = Some(LastSystemUpdate::Committed { version: "v1".into() });
        assert_eq!(phase_of(Some(&status(false, false, older)), "v2"), Phase::Restarting);
        let done = Some(LastSystemUpdate::Committed { version: "v2".into() });
        assert_eq!(phase_of(Some(&status(false, false, done)), "v2"), Phase::Committed);
        let failed = Some(LastSystemUpdate::RolledBack { failed: "v2".into() });
        assert_eq!(phase_of(Some(&status(false, false, failed)), "v2"), Phase::RolledBack);
    }
}
