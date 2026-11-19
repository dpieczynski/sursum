// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing a remote with the PIN, and the device list. A paired device holds a cookie and may do
//! everything (ADR 0024); the screen needs none.

use serde::{Deserialize, Serialize};

/// What a page needs to know before connecting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// True once a PIN has been set on the device.
    pub pin_required: bool,
    /// True if this browser is paired (or the server runs open for development).
    pub paired: bool,
    /// True while no PIN is set: the first remote must set one (`POST /api/setup`) and nobody
    /// may control the screen until then, so a freshly installed device is never open.
    #[serde(default)]
    pub setup_needed: bool,
    /// What this server is: a device with a screen, or a library without one. Older servers
    /// do not send it and are devices.
    #[serde(default)]
    pub installation: Installation,
}

/// What kind of installation a server is, so the remote offers only what makes sense there.
/// The server accepts the same requests either way; this decides what is shown, not what is
/// allowed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Installation {
    /// A device driving a screen (the Raspberry Pi next to the projector): the whole remote.
    #[default]
    Screen,
    /// The desktop program at home (ADR 0020): songs, the editor and the library file; no
    /// screen, no service, no pairing.
    Library,
}

/// One paired remote, for the device list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceView {
    pub device_id: String,
    pub name: String,
    /// When it was paired (server clock in UTC, `YYYY-MM-DD HH:MM:SS`; the clock may be wrong
    /// offline). The remote shows it in its own local time.
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
