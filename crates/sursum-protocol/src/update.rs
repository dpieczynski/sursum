// SPDX-License-Identifier: GPL-3.0-or-later
//! Application updates from the remote (signed `.sursum-app` bundles, ADR 0013): what the device
//! runs, how the last update ended, and the answer to an upload.

use serde::{Deserialize, Serialize};

/// `GET /api/update`: the running version and the state of updates on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Build id of the running server (also shown when it does not run from a release).
    pub build: String,
    /// The installed release that runs (`current`), when the device uses releases.
    pub release: Option<String>,
    /// The running release is still on trial: it goes back by itself if it keeps failing.
    pub on_trial: bool,
    /// How the most recent update ended, if any.
    pub last: Option<LastUpdate>,
    /// Whether this server can take an update from the remote at all (signed builds under a
    /// service manager only), so the remote does not offer what would be refused.
    pub can_install: bool,
    /// The device's system (the read-only A/B image, ADR 0019), on devices built from the image.
    #[serde(default)]
    pub system: Option<SystemStatus>,
}

/// The read-only system of an imaged device and its updates (`.sursum-system`, ADR 0019).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemStatus {
    /// The running system's version (the image's build id).
    pub version: String,
    /// The running system was just installed and is not yet confirmed: if it does not come up
    /// healthy, the device restarts into the previous system by itself.
    pub on_trial: bool,
    /// A system update is being written to the other slot, or written and about to restart into
    /// it: the device will restart any moment.
    #[serde(default)]
    pub installing: bool,
    /// How the most recent system update ended, if any.
    pub last: Option<LastSystemUpdate>,
    /// Whether a system update can be uploaded now.
    pub can_install: bool,
}

/// How the most recent system update ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LastSystemUpdate {
    /// The new system came up healthy and is now the one the device starts.
    Committed { version: String },
    /// The new system did not come up healthy (or could not be written); the device runs the
    /// system it had before.
    RolledBack { failed: String },
}

/// How the most recent update ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LastUpdate {
    /// The new release ran healthily through its trial and stays.
    Committed { version: String },
    /// The new release kept failing, so the device went back to the release before it.
    RolledBack {
        failed: String,
        restored: String,
        /// The database was put back to the copy taken before the update (the new release had
        /// already changed its structure); changes made during the failed trial are gone.
        database_restored: bool,
    },
}

/// The answer to `POST /api/update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InstallOutcome {
    /// Installed; the server restarts into it right after answering.
    Installed { version: String },
    /// Nothing was changed.
    Refused { reason: InstallRefusal },
}

/// Why an uploaded bundle was not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRefusal {
    /// This build was made without a bundle key and accepts no updates.
    NoKey,
    /// The server does not run under a service manager that would start it again.
    NotSupervised,
    /// Not a bundle, damaged, or not signed with this device's key.
    BadBundle,
    /// That version already runs.
    AlreadyCurrent,
    /// Writing the release failed (e.g. a full disk); the old release keeps running.
    Failed,
    /// Another update is being installed or is on trial; try again once it has ended.
    Busy,
}
