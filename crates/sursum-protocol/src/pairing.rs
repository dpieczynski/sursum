// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing a remote with the PIN, admin mode and the device list. A paired device holds a
//! cookie; the screen needs none.

use serde::{Deserialize, Serialize};

/// What a page needs to know before connecting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// True once a PIN has been set on the device.
    pub pin_required: bool,
    /// True if this browser is paired (or the server runs open for development).
    pub paired: bool,
    /// True while admin mode is unlocked here: import, merge, calibration and the device list.
    #[serde(default)]
    pub admin: bool,
    /// True while no PIN is set: the first remote must set one (`POST /api/setup`) and nobody
    /// may control the screen until then, so a freshly installed device is never open.
    #[serde(default)]
    pub setup_needed: bool,
}

/// `POST /api/admin`: unlock admin mode on this (paired) device for a while.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRequest {
    pub pin: String,
}

/// One paired remote, for the device list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceView {
    pub device_id: String,
    pub name: String,
    /// When it was paired (server time, `YYYY-MM-DD HH:MM:SS`; the clock may be wrong offline).
    pub paired_at: String,
    /// True for the device asking (so nobody revokes their own tablet by mistake).
    pub this_device: bool,
}

/// `POST /api/pair`, and `POST /api/setup` (where `pin` is the new PIN).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRequest {
    pub pin: String,
    /// A name for the device list, e.g. "Tablet organisty".
    pub name: String,
}

/// Result of a pairing attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PairOutcome {
    Paired,
    /// Admin mode is unlocked (answer to `POST /api/admin`).
    Unlocked,
    WrongPin,
    /// Setup only: the new PIN is not 6 to 12 digits.
    InvalidPin,
    /// Setup only: another remote set the PIN first; pair with it instead.
    AlreadySet,
    /// Too many wrong attempts from this address; try again after the given time.
    Locked {
        retry_after_s: u32,
    },
}
