// SPDX-License-Identifier: GPL-3.0-or-later
//! How text slides look. One style for every screen: it used to travel in every snapshot
//! (`ScreenState.theme`), but nothing ever set anything but the default, so it lives here with
//! the drawing code. A style chosen per device would come back as data from the server.

/// Visual style used to draw text slides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// CSS font family of a bundled font (`assets/fonts/`).
    pub font_family: &'static str,
    pub font_weight: u16,
    /// CSS colour of the text.
    pub text_color: &'static str,
    /// Line height in thousandths of the font size (1250 = 1.25).
    pub line_height_permille: u16,
    /// Smallest font size the fitter may choose, in scene pixels.
    pub min_font_px: u32,
    /// Largest font size the fitter may choose, in scene pixels.
    pub max_font_px: u32,
}

/// The style of every screen and preview: centred white Lato Bold on black.
pub const THEME: Theme = Theme {
    font_family: "Lato",
    font_weight: 700,
    text_color: "#ffffff",
    line_height_permille: 1250,
    min_font_px: 36,
    max_font_px: 120,
};
