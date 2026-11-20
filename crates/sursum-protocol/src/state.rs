// SPDX-License-Identifier: GPL-3.0-or-later
//! Snapshots the server sends. Clients replace their whole state with each snapshot.

use serde::{Deserialize, Serialize};

use crate::ids::{AssetId, ChurchId, ItemId, Rev, SetId, SongId};
use crate::layout::{ScreenLayout, ScreenProfile};
use crate::slide::Deck;

/// The layer above content: nothing, black, or the parish logo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlay {
    #[default]
    None,
    Black,
    Logo,
}

/// A deck with the position currently shown (or proposed as next).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSlide {
    pub deck: Deck,
    pub slide_index: u32,
    /// The gap after the item's last slide: the screen shows an empty slide (black background)
    /// until "next" moves on, so the next song's words do not appear before it starts.
    #[serde(default)]
    pub gap: bool,
}

/// What "next" does from here, so the remote can say it before the operator presses it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    /// The next slide of the same item.
    Slide,
    /// The gap after the last slide (an empty screen).
    Gap,
    /// The first slide of the next item of the set.
    Item,
    /// Nothing is shown yet: "next" starts the set.
    Start,
    /// Nothing comes after this: "next" does nothing.
    #[default]
    End,
}

/// What "next" does after an item's last slide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndOfItem {
    /// An empty screen first, then the next item (like empty slides between songs in OpenSong).
    #[default]
    Gap,
    /// Straight to the next item's first slide.
    Continue,
}

/// Everything the audience screen needs; nothing about sets or editing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenState {
    pub rev: Rev,
    pub live: Option<LiveSlide>,
    /// The next item's deck, so the screen can fit its font size ahead of time.
    pub upcoming: Option<Deck>,
    pub overlay: Overlay,
    /// Crossfade duration; 0 means an instant cut.
    pub transition_ms: u32,
    pub layout: ScreenLayout,
    /// While set, the screen draws the calibration pattern for this profile on top of everything.
    #[serde(default)]
    pub calibration: Option<ScreenProfile>,
    /// The parish logo shown by [`Overlay::Logo`]; without one the logo overlay is plain black.
    #[serde(default)]
    pub logo: Option<AssetId>,
}

/// One entry of the active set as the remote lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetItemView {
    pub item_id: ItemId,
    /// 1-based position, used for "go to item N".
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub kind: SetItemKind,
    /// The song behind a song item (to open it in the editor or mark it as already in the set).
    #[serde(default)]
    pub song_id: Option<SongId>,
    /// The song's number in its songbook, if any.
    #[serde(default)]
    pub song_number: Option<String>,
}

/// What a set item is, so the remote can mark it (and explain a song that no longer exists).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetItemKind {
    #[default]
    Song,
    Text,
    Image,
    Pause,
    /// The song was deleted from the library; the item cannot be shown any more.
    MissingSong,
}

/// A set in the list of sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSummary {
    pub set_id: SetId,
    pub name: String,
    pub items: u32,
    /// When the service is (ADR 0025); the list is ordered by it.
    #[serde(default)]
    pub date: Option<String>,
    /// Changed in two places and not reviewed yet (ADR 0025): marked in the list.
    #[serde(default)]
    pub review: bool,
}

/// The active set as the remote shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetView {
    pub set_id: SetId,
    pub name: String,
    pub items: Vec<SetItemView>,
    /// When the service is (ADR 0025).
    #[serde(default)]
    pub date: Option<String>,
    /// Changed in two places and not reviewed yet: the Zestaw view offers the comparison.
    #[serde(default)]
    pub review: bool,
}

/// A song as listed in search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongSummary {
    pub song_id: SongId,
    pub title: String,
    /// Number in a songbook, if any.
    pub number: Option<String>,
    pub first_line: String,
    /// Changed in two places and not reviewed yet (ADR 0025): "do przejrzenia".
    #[serde(default)]
    pub review: bool,
}

/// Power state of the projector or monitor (ADR 0012).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DisplayPowerState {
    #[default]
    Unknown,
    On,
    Standby,
    WarmingUp,
    CoolingDown,
    Unavailable {
        reason: String,
    },
}

/// Everything an operator's remote needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteState {
    pub rev: Rev,
    /// Exactly what the screen shows (the program).
    pub program: ScreenState,
    /// What "next" would show.
    pub next: Option<LiveSlide>,
    pub active_set: Option<SetView>,
    /// All sets, for switching.
    pub sets: Vec<SetSummary>,
    /// True while the last removal can be undone.
    pub can_undo: bool,
    pub display_power: DisplayPowerState,
    /// The profile being used, for the calibration panel.
    #[serde(default)]
    pub screen_profile: ScreenProfile,
    /// True while a screen (the kiosk browser) is connected.
    #[serde(default)]
    pub screen_connected: bool,
    /// What "next" does from here.
    #[serde(default)]
    pub next_step: NextStep,
    /// What "next" does after an item's last slide (a device setting).
    #[serde(default)]
    pub end_of_item: EndOfItem,
    /// How many songs were changed in two places and wait for review (ADR 0025).
    #[serde(default)]
    pub songs_to_review: u32,
    /// The church this device serves; `None` until it is named. In the desktop program, the
    /// church whose sets the Zestaw view lists (`None` until one is chosen: every set).
    #[serde(default)]
    pub church: Option<ChurchView>,
    /// Every church this device knows (its own and those from library files), by name.
    #[serde(default)]
    pub churches: Vec<ChurchView>,
}

/// A church of the parish (ADR 0025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChurchView {
    pub church_id: ChurchId,
    pub name: String,
}
