// SPDX-License-Identifier: GPL-3.0-or-later
//! The Msza view: what is on screen, the slides of the live item, the running order and the
//! controls, built for one hand and a glance. Nothing here edits anything; a tap shows.

pub mod info;
mod now;
mod pad;
mod screen_sheet;
mod search_sheet;
mod set;
mod slides;
mod strip;

pub use now::Now;
pub use pad::Pad;
pub use screen_sheet::ScreenSheet;
pub use search_sheet::{SearchSheet, search_results};
pub use set::LiveSet;
pub use slides::Slides;
pub use strip::LiveStrip;
