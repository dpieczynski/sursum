// SPDX-License-Identifier: GPL-3.0-or-later
//! Snapshots the server sends. Clients replace their whole state with each snapshot.

use serde::{Deserialize, Serialize};

use crate::ids::{AssetId, ItemId, Rev, SetId, SongId};
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

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Center,
    Left,
}

/// Visual style used to draw slides.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Theme {
    /// CSS font family of a bundled font, e.g. `Lato`.
    pub font_family: String,
    pub font_weight: u16,
    /// CSS colour of the text.
    pub text_color: String,
    /// Line height in thousandths of the font size (1250 = 1.25).
    pub line_height_permille: u16,
    pub text_align: TextAlign,
    /// Smallest font size the fitter may choose, in scene pixels.
    pub min_font_px: u32,
    /// Largest font size the fitter may choose, in scene pixels.
    pub max_font_px: u32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            font_family: "Lato".to_owned(),
            font_weight: 700,
            text_color: "#ffffff".to_owned(),
            line_height_permille: 1250,
            text_align: TextAlign::Center,
            min_font_px: 36,
            max_font_px: 120,
        }
    }
}

/// A deck with the position currently shown (or proposed as next).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSlide {
    pub deck: Deck,
    pub slide_index: u32,
}

/// Everything the audience screen needs; nothing about sets or editing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenState {
    pub rev: Rev,
    pub live: Option<LiveSlide>,
    /// The next item's deck, so the screen can fit its font size ahead of time.
    pub upcoming: Option<Deck>,
    pub overlay: Overlay,
    pub theme: Theme,
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

/// A jump button for one section of the live deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionJump {
    pub label: String,
    pub slide_index: u32,
}

/// One entry of the active set as the remote lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetItemView {
    pub item_id: ItemId,
    /// 1-based position, used for "go to item N".
    pub number: u32,
    pub title: String,
}

/// A set in the list of sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSummary {
    pub set_id: SetId,
    pub name: String,
    pub items: u32,
}

/// The active set as the remote shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetView {
    pub set_id: SetId,
    pub name: String,
    pub items: Vec<SetItemView>,
}

/// A song as listed in search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongSummary {
    pub song_id: SongId,
    pub title: String,
    /// Number in a songbook, if any.
    pub number: Option<String>,
    pub first_line: String,
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
    /// Jump buttons for the live deck.
    pub sections: Vec<SectionJump>,
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
}
