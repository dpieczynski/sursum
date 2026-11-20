// SPDX-License-Identifier: GPL-3.0-or-later
//! The locked progress sheet while the device updates itself (ADR 0019, ADR 0022). Nothing else
//! in the remote should look usable while the device is about to restart, so the sheet covers
//! every view and cannot be closed until the update has ended.
//!
//! It starts when an upload is accepted ([`Maintenance::start`]), and also on its own whenever the
//! page loads or the connection comes back while an update is in progress: the new version
//! reloads the remote (new build), and other remotes may connect mid-update. It asks for the state
//! every 3 s and shows the step; the server's state decides everything.

use leptos::prelude::*;
use sursum_client::Status;
use sursum_protocol::{LastSystemUpdate, UpdateStatus};

use crate::api::get_json;
use crate::messages::{Msg, installed_version, pl};
use crate::ui::Sheet;

/// Where a followed update is, as far as the remote can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The update is being written to the other slot.
    Writing,
    /// No answer from the server: it (or the device) is restarting.
    Restarting,
    /// The new version runs on trial.
    Checking,
    Committed,
    RolledBack,
}

impl Phase {
    fn finished(self) -> bool {
        matches!(self, Phase::Committed | Phase::RolledBack)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Followed {
    version: String,
    phase: Phase,
}

/// The followed update, shared through the context.
#[derive(Clone, Copy)]
pub struct Maintenance(RwSignal<Option<Followed>>);

impl Maintenance {
    /// Nothing followed yet; the app provides one through the context, so install buttons in any
    /// view and the sheet share it.
    pub fn new() -> Self {
        Maintenance(RwSignal::new(None))
    }

    /// Follows the update to `version` until it has ended (after an accepted upload).
    pub fn start(self, version: String) {
        let following = self.0.with_untracked(Option::is_some);
        self.0.set(Some(Followed { version, phase: Phase::Writing }));
        if !following {
            leptos::task::spawn_local(follow(self.0));
        }
    }
}

/// The version of the update in progress according to `status`, if any.
fn in_progress(status: &UpdateStatus) -> Option<String> {
    let system = status.system.as_ref()?;
    (system.installing || system.on_trial).then(|| system.target.clone().unwrap_or_default())
}

/// The phase of the update to `version` from a status answer (`None`: no answer).
fn phase_of(status: Option<&UpdateStatus>, version: &str) -> Phase {
    let Some(system) = status.and_then(|status| status.system.as_ref()) else { return Phase::Restarting };
    match &system.last {
        _ if system.installing => Phase::Writing,
        _ if system.on_trial => Phase::Checking,
        Some(LastSystemUpdate::Committed { version: done }) if done == version => Phase::Committed,
        Some(LastSystemUpdate::RolledBack { failed, .. }) if failed == version => Phase::RolledBack,
        _ => Phase::Restarting,
    }
}

/// Asks for the state every 3 s until the followed update has ended.
async fn follow(followed: RwSignal<Option<Followed>>) {
    loop {
        gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
        let Some(current) = followed.get_untracked() else { return };
        let status: Option<UpdateStatus> = get_json("/api/update/status").await;
        let phase = phase_of(status.as_ref(), &current.version);
        followed.set(Some(Followed { phase, ..current }));
        if phase.finished() {
            return;
        }
    }
}

/// Shows the sheet of `maintenance`; checks for an update in progress whenever the connection is
/// (re)established.
#[component]
pub fn MaintenanceSheet(connection: RwSignal<Status>, maintenance: Maintenance) -> impl IntoView {
    let followed = maintenance.0;
    Effect::new(move |_| {
        if connection.get() == Status::Connected && followed.with_untracked(Option::is_none) {
            leptos::task::spawn_local(async move {
                let status: Option<UpdateStatus> = get_json("/api/update/status").await;
                if let Some(version) = status.as_ref().and_then(in_progress) {
                    maintenance.start(version);
                }
            });
        }
    });
    let locked = move || followed.with(|followed| followed.as_ref().is_some_and(|f| !f.phase.finished()));
    let close = move || followed.set(None);
    move || {
        followed.get().map(|current| {
            let lines = sheet_lines(&current).into_iter().map(|line| view! { <p>{line}</p> }).collect_view();
            view! {
                <Sheet
                    title=pl(Msg::SystemUpdateTitle).to_owned()
                    on_close=close
                    testid="maintenance"
                    locked=Signal::derive(locked)
                >
                    <Show when=locked>
                        <div class="progress-spinner" role="progressbar" aria-busy="true"></div>
                    </Show>
                    <div class="sheet-status" data-testid="maintenance-status">
                        {lines}
                    </div>
                </Sheet>
            }
        })
    }
}

fn sheet_lines(followed: &Followed) -> Vec<String> {
    let now = match followed.phase {
        Phase::Writing => Msg::SystemUpdateWriting,
        Phase::Restarting => Msg::SystemUpdateRestarting,
        Phase::Checking => Msg::SystemUpdateChecking,
        Phase::Committed => Msg::SystemUpdateCommitted,
        Phase::RolledBack => Msg::SystemUpdateRolledBack,
    };
    let mut lines = Vec::new();
    if !followed.version.is_empty() {
        lines.push(installed_version(&followed.version));
    }
    lines.push(pl(now).to_owned());
    if !followed.phase.finished() {
        lines.push(pl(Msg::SystemUpdateDontSwitchOff).to_owned());
    }
    lines
}

#[cfg(test)]
mod tests {
    use sursum_protocol::SystemStatus;

    use super::*;

    fn status(installing: bool, on_trial: bool, last: Option<LastSystemUpdate>) -> UpdateStatus {
        let system = SystemStatus {
            version: "v1".into(),
            on_trial,
            installing,
            target: (installing || on_trial).then(|| "v2".into()),
            last,
            can_install: false,
        };
        UpdateStatus { build: "v1".into(), system: Some(system) }
    }

    #[test]
    fn an_update_is_followed_from_writing_to_its_end() {
        let phase = |status: Option<&UpdateStatus>| phase_of(status, "v2");
        assert_eq!(phase(Some(&status(true, false, None))), Phase::Writing);
        assert_eq!(phase(None), Phase::Restarting);
        assert_eq!(phase(Some(&status(false, true, None))), Phase::Checking);
        let older = Some(LastSystemUpdate::Committed { version: "v1".into() });
        assert_eq!(phase(Some(&status(false, false, older))), Phase::Restarting);
        let done = Some(LastSystemUpdate::Committed { version: "v2".into() });
        assert_eq!(phase(Some(&status(false, false, done))), Phase::Committed);
        let failed = Some(LastSystemUpdate::RolledBack { failed: "v2".into(), database_restored: true });
        assert_eq!(phase(Some(&status(false, false, failed))), Phase::RolledBack);
    }

    #[test]
    fn a_remote_that_connects_mid_update_finds_it() {
        assert_eq!(in_progress(&status(false, false, None)), None);
        assert_eq!(in_progress(&status(true, false, None)), Some("v2".into()));
        assert_eq!(in_progress(&status(false, true, None)), Some("v2".into()));
        assert_eq!(in_progress(&UpdateStatus { build: "dev".into(), system: None }), None);
    }
}
