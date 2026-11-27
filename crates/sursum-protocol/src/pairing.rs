// SPDX-License-Identifier: GPL-3.0-or-later
//! Pairing a remote with the PIN, and the device list. A paired device holds a cookie and may do
//! everything (ADR 0024); the screen needs none. Also the server's identity, with which the
//! Android launcher keeps its pairing when the device gets a new address (ADR 0028).

use serde::{Deserialize, Serialize};

use crate::ServerId;

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
    /// Setup only: the name of the church this device serves, e.g. "Tulce" (ADR 0025).
    #[serde(default)]
    pub church: String,
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
    /// Setup only: a device with a screen needs the name of its church.
    ChurchMissing,
    /// Too many wrong attempts from this address; try again after the given time.
    Locked {
        retry_after_s: u32,
    },
}

/// `GET /api/identity` (open to the local network): which sursum server answers here. The
/// Android launcher remembers it with the address it loaded the remote from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerIdentity {
    pub server_id: ServerId,
}

/// `POST /api/identity/proof` (open to the local network): asks the server to prove that it
/// holds a pairing, before the launcher moves that pairing's cookie to the server's new address.
/// Neither field is secret: a host that only pretends to be the device learns nothing it could
/// use from them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingProofRequest {
    /// Names the pairing without revealing it: lowercase hex of
    /// SHA-256(`"sursum-pairing-key\n"` ‖ token hash), where the token hash is the lowercase hex
    /// SHA-256 of the cookie's value (what the server stores).
    pub key: String,
    /// Fresh random lowercase hex from the launcher (32 to 128 characters), so an old answer
    /// cannot be replayed.
    pub nonce: String,
}

/// Answer to [`PairingProofRequest`] (404 when no paired remote has that key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingProof {
    pub server_id: ServerId,
    /// The address and port the request reached, as the server saw it (`192.168.1.20:80`,
    /// `[fd00::5]:80`). The launcher accepts the proof only for the address it connected to,
    /// so a host that relays the question to the real device cannot reuse the answer.
    pub address: String,
    /// Lowercase hex HMAC-SHA256, keyed with the token hash (as ASCII hex), of
    /// `"sursum-pairing-proof\n" ‖ server_id ‖ "\n" ‖ address ‖ "\n" ‖ nonce`.
    pub proof: String,
}
