// SPDX-License-Identifier: GPL-3.0-or-later
//! Geometry of the fixed 1920×1080 stage and the per-device screen profile (ADR 0008).

use serde::{Deserialize, Serialize};

/// Width of the logical stage in scene pixels. Every screen scales this stage to fit.
pub const SCENE_WIDTH: u32 = 1920;
/// Height of the logical stage in scene pixels.
pub const SCENE_HEIGHT: u32 = 1080;

/// Axis-aligned rectangle in scene pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The whole stage.
    pub const SCENE: Rect = Rect { x: 0, y: 0, width: SCENE_WIDTH, height: SCENE_HEIGHT };

    /// Creates a rectangle from its top-left corner and size.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Rect { x, y, width, height }
    }

    /// Exclusive right edge.
    pub const fn right(&self) -> u32 {
        self.x + self.width
    }

    /// Exclusive bottom edge.
    pub const fn bottom(&self) -> u32 {
        self.y + self.height
    }

    /// Area in square scene pixels.
    pub const fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// True when the two rectangles share any area (touching edges do not count).
    pub const fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }

    /// True when `other` lies completely inside `self`.
    pub const fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

/// Margins in scene pixels, used for the overscan safe area.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Insets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl Insets {
    /// The same margin on every side.
    pub const fn uniform(value: u32) -> Self {
        Insets { top: value, right: value, bottom: value, left: value }
    }
}

/// What to paint over an exclusion zone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskMode {
    /// Paint the zone black so nothing lights up the obstacle (default).
    #[default]
    Black,
    /// Keep text out of the zone but leave the background visible.
    None,
}

/// A part of the frame that is unusable for text, e.g. a dark pillar the projector hits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExclusionZone {
    /// Human-readable name shown in the calibration panel.
    pub name: String,
    pub rect: Rect,
    #[serde(default)]
    pub mask: MaskMode,
}

/// Physical description of one screen: which part of the frame the congregation can read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenProfile {
    pub safe_area: Insets,
    #[serde(default)]
    pub exclusions: Vec<ExclusionZone>,
}

/// Layout computed by the server from a [`ScreenProfile`]: where text may go and what to mask.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenLayout {
    /// Rectangle for text and the logo.
    pub content: Rect,
    /// Black rectangles drawn above content (below the black/logo overlay).
    pub masks: Vec<Rect>,
}
