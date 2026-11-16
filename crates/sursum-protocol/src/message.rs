// SPDX-License-Identifier: GPL-3.0-or-later
//! WebSocket messages. JSON text frames, tagged with `"t"`. See `docs/protocol.md`.

use serde::{Deserialize, Serialize};

use crate::ids::{ClientId, ItemId, Rev, SongId};
use crate::state::{Overlay, RemoteState, ScreenState};

/// Bumped on any incompatible change; a client with another version reloads itself.
pub const PROTOCOL_VERSION: u16 = 1;

/// Which kind of client is connecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The audience screen: read-only.
    Screen,
    /// An operator's remote.
    Remote,
}

/// Messages from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message after connecting.
    Hello { protocol: u16, role: Role, client_id: ClientId, build_id: String },
    /// An operator command. `base_rev` is the revision the operator saw when acting.
    Cmd { seq: u32, base_rev: Rev, cmd: Command },
    /// Keep-alive; the server answers with [`ServerMsg::Pong`].
    Ping,
    /// A client-side problem worth logging on the server (never shown on the screen).
    Log { message: String },
}

/// Operator intents. The server owns all logic; commands only state what the operator wants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Next slide (relative; subject to the `Stale` rule).
    Next,
    /// Previous slide (relative).
    Prev,
    /// First slide of the next item (relative).
    NextItem,
    /// First slide of the previous item (relative).
    PrevItem,
    /// Show a specific slide of a specific item (absolute).
    GoTo { item_id: ItemId, slide_index: u32 },
    /// Show the first slide of the N-th item of the set, 1-based (absolute).
    GoToItemNumber { number: u32 },
    /// Show a slide of the live item by index, e.g. from a section chip (absolute).
    GoToSlide { slide_index: u32 },
    /// Set the overlay (absolute).
    SetOverlay { overlay: Overlay },
    /// Black on/off in one tap (absolute).
    ToggleBlack,
    /// Insert a song right after the live item and show it at once (absolute).
    ShowNow { song_id: SongId },
    /// Append a song to the end of the active set (does not change the screen).
    AddSong { song_id: SongId },
}

impl Command {
    /// Relative commands depend on what the operator saw and may be rejected as stale.
    pub fn is_relative(&self) -> bool {
        matches!(self, Command::Next | Command::Prev | Command::NextItem | Command::PrevItem)
    }
}

/// Why a command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    NoActiveSet,
    Forbidden,
    Unsupported,
}

/// Result of one command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CmdOutcome {
    Applied {
        rev: Rev,
    },
    /// Someone else changed the program since `base_rev`; nothing was done.
    Stale {
        rev: Rev,
    },
    Rejected {
        code: ErrorCode,
    },
}

/// Messages from the server to a client.
// Snapshots are much larger than the other variants; messages are short-lived (serialized and
// dropped), so boxing would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome { protocol: u16, build_id: String },
    Screen { state: ScreenState },
    Remote { state: RemoteState },
    CmdResult { seq: u32, outcome: CmdOutcome },
    Pong,
}
