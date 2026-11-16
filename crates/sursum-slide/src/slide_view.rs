// SPDX-License-Identifier: GPL-3.0-or-later
//! One slide, drawn inside a 1920×1080 stage.

use leptos::prelude::*;
use sursum_protocol::{Background, Rect, Slide, SlideBody, TextAlign, Theme};

/// CSS classes of the text box; shared with the measuring code in `fit.rs`.
pub fn lines_class(theme: &Theme, wrap: bool) -> String {
    let align = match theme.text_align {
        TextAlign::Center => "align-center",
        TextAlign::Left => "align-left",
    };
    let wrap = if wrap { " wrap" } else { "" };
    format!("sursum-lines {align}{wrap}")
}

/// Inline style of the text box; shared with the measuring code in `fit.rs`, so the measured
/// box and the visible box are laid out identically.
pub fn text_box_style(theme: &Theme, content: Rect, font_px: u32) -> String {
    format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;font-family:\"{}\";font-weight:{};font-size:{}px;line-height:{};color:{};",
        content.x,
        content.y,
        content.width,
        content.height,
        theme.font_family,
        theme.font_weight,
        font_px,
        f64::from(theme.line_height_permille) / 1000.0,
        theme.text_color,
    )
}

/// CSS for an opaque layer background.
pub fn background_style(background: &Background) -> String {
    match background {
        Background::Color { css } => format!("background:{css};"),
    }
}

/// Draws one slide's content into the given rectangle.
#[component]
pub fn SlideView(slide: Slide, theme: Theme, content: Rect, font_px: u32, wrap: bool) -> impl IntoView {
    let lines = match slide.body {
        SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => lines,
        // Images arrive in milestone M3.
        SlideBody::Image { .. } => Vec::new(),
    };
    view! {
        <div class=lines_class(&theme, wrap) style=text_box_style(&theme, content, font_px)>
            {lines.into_iter().map(|line| view! { <div class="sursum-line">{line}</div> }).collect_view()}
        </div>
    }
}
