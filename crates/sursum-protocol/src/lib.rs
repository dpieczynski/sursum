// SPDX-License-Identifier: GPL-3.0-or-later
//! Wire types shared by the sursum server (native) and its clients (WebAssembly).
//!
//! This crate holds **data only**: serde types and constructors, no domain logic, no I/O.
//! It must compile for both the native target and `wasm32-unknown-unknown`, which is why it
//! depends on nothing but `serde`.

mod editor;
mod ids;
mod layout;
mod message;
mod pairing;
mod slide;
mod state;

pub use editor::{DraftPreview, ImportReport, MergeReport, SaveOutcome, SongDraft};
pub use ids::{AssetId, ClientId, DeckId, ItemId, Rev, SetId, SlideKey, SongId};
pub use layout::{
    ExclusionZone, Insets, MaskMode, Rect, SCENE_HEIGHT, SCENE_WIDTH, ScreenLayout, ScreenProfile,
};
pub use message::{
    ClientMsg, CmdOutcome, Command, ErrorCode, PROTOCOL_VERSION, PowerAction, Role, ServerMsg,
};
pub use pairing::{AdminRequest, DeviceView, PairOutcome, PairRequest, SessionInfo};
pub use slide::{Background, Deck, ImageFit, Slide, SlideBody};
pub use state::{
    DisplayPowerState, LiveSlide, Overlay, RemoteState, ScreenState, SectionJump, SetItemView, SetSummary,
    SetView, SongSummary, TextAlign, Theme,
};
