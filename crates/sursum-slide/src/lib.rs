// SPDX-License-Identifier: GPL-3.0-or-later
//! Slide rendering shared by the audience screen and the remote preview.
//!
//! This crate is the **only** place that draws a slide, so the preview on the remote is the
//! same component as the screen. It contains no domain logic: what to show comes from the
//! server; this crate decides only how big the text can be (that needs font metrics).
//!
//! - [`fit`]: choosing one font size per deck (pure algorithm + DOM measuring).
//! - [`stage`]: the fixed 1920×1080 stage scaled to its container.
//! - [`slide_view`]: one slide.
//! - [`theme`]: the one fixed text style.
//! - [`layered`]: the screen: two crossfading layers, masks and the black/logo overlay.
//! - [`preview`]: a static preview for the remote.

// Leptos components take their props by value; clippy's pass-by-value lint does not fit them.
#![allow(clippy::needless_pass_by_value)]

pub mod calibration;
pub mod fit;
pub mod layered;
pub mod preview;
pub mod shown;
pub mod slide_view;
pub mod stage;
pub mod theme;

mod dom;

pub use calibration::CalibrationPattern;
pub use layered::LayeredStage;
pub use preview::{PreviewData, SlidePreview};
pub use shown::shown_slide;
pub use stage::Stage;
