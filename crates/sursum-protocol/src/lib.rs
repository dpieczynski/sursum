// SPDX-License-Identifier: GPL-3.0-or-later
//! Wire types shared by the sursum server (native) and its clients (WebAssembly).
//!
//! This crate holds **data only**: serde types and constructors, no domain logic, no I/O.
//! It must compile for both the native target and `wasm32-unknown-unknown`, which is why it
//! depends on nothing but `serde`.

mod backup;
mod editor;
mod ids;
mod layout;
mod message;
mod pairing;
mod pictures;
mod slide;
mod state;
mod update;

pub use backup::{BackupSummary, RestoreOutcome, RestoreRefusal};
pub use editor::{
    DraftPreview, ImportProblem, ImportProblemCode, ImportReport, KeepVersion, MarkupWarning,
    MarkupWarningCode, MergeReport, SaveOutcome, SetVersionItem, SetVersionView, SetVersions, SongDraft,
    SongVersionView, SongVersions,
};
pub use ids::{AssetId, ChurchId, ClientId, DeckId, ItemId, Rev, ServerId, SetId, SlideKey, SongId};
pub use layout::{
    ExclusionZone, Insets, MaskMode, Rect, SCENE_HEIGHT, SCENE_WIDTH, ScreenLayout, ScreenProfile,
};
pub use message::{
    ClientMsg, CmdOutcome, Command, ErrorCode, PROTOCOL_VERSION, PowerAction, Role, ServerMsg,
};
pub use pairing::{
    DeviceView, Installation, PairOutcome, PairRequest, PairingProof, PairingProofRequest, ServerIdentity,
    SessionInfo,
};
pub use pictures::{PictureCleanup, PictureCleanupRequest};
pub use slide::{Deck, SectionKind, SectionName, Slide, SlideBody, SlidePart};
pub use state::{
    ChurchView, DisplayPowerState, EndOfItem, LiveSlide, NextStep, Overlay, RemoteState, ScreenState,
    SetItemKind, SetItemView, SetSummary, SetView, SongSummary,
};
pub use update::{InstallOutcome, InstallRefusal, LastSystemUpdate, SystemStatus, UpdateStatus};
