// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates from the remote (signed `.sursum-system` files, ADR 0019 and ADR 0022): what the device
//! runs, how the last update ended, and the answer to an upload. There is one update path: the
//! server is part of the system, so a device has one version.

use serde::{Deserialize, Serialize};

/// `GET /api/update/status`: the running version and the state of updates on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Build id of the running server. On a device it equals the system's version, unless a
    /// developer runs another build for a while (`just try`, until the next restart).
    pub build: String,
    /// The device's system (the read-only A/B image), on devices built from the image; `None`
    /// elsewhere (the desktop program, development), which take no updates.
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
    /// The version being installed or on trial, so a remote that (re)connects meanwhile can
    /// follow it to its end.
    #[serde(default)]
    pub target: Option<String>,
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
    RolledBack {
        failed: String,
        /// The database was put back to the copy taken before the update (the new system had
        /// already changed its structure); changes made during the failed trial are gone.
        #[serde(default)]
        database_restored: bool,
    },
}

/// The answer to `POST /api/system/update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InstallOutcome {
    /// Installed; the server restarts into it right after answering.
    Installed { version: String },
    /// Nothing was changed.
    Refused { reason: InstallRefusal },
}

/// Why an uploaded update was not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRefusal {
    /// This build was made without the update key and accepts no updates.
    NoKey,
    /// Not a device built from the sursum image, or not under its service manager.
    NotSupervised,
    /// Not an update file, damaged, or not signed with this device's key.
    BadBundle,
    /// That version already runs.
    AlreadyCurrent,
    /// Receiving or staging the update failed (e.g. a full disk); nothing changed.
    Failed,
    /// Another update is being installed or is on trial; try again once it has ended.
    Busy,
}
