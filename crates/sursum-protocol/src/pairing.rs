// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing a remote with the PIN. A paired device holds a cookie; the screen needs none.

use serde::{Deserialize, Serialize};

/// What a page needs to know before connecting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// True once a PIN has been set on the device; until then control is open.
    pub pin_required: bool,
    /// True if this browser is paired (or no PIN is required).
    pub paired: bool,
}

/// `POST /api/pair`.
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
    /// Too many wrong attempts from this address; try again after the given time.
    Locked {
        retry_after_s: u32,
    },
}
