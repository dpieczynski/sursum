// SPDX-License-Identifier: GPL-3.0-or-later
//! One slide, drawn inside a 1920×1080 stage.

use leptos::prelude::*;
use sursum_protocol::{AssetId, Rect, Slide, SlideBody};

use crate::theme::THEME;

/// CSS classes of the text box; shared with the measuring code in `fit.rs`.
pub fn lines_class(wrap: bool) -> &'static str {
    if wrap { "sursum-lines wrap" } else { "sursum-lines" }
}

/// Inline style of the text box; shared with the measuring code in `fit.rs`, so the measured
/// box and the visible box are laid out identically.
pub fn text_box_style(content: Rect, font_px: u32) -> String {
    format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;font-family:\"{}\";font-weight:{};font-size:{}px;line-height:{};color:{};",
        content.x,
        content.y,
        content.width,
        content.height,
        THEME.font_family,
        THEME.font_weight,
        font_px,
        f64::from(THEME.line_height_permille) / 1000.0,
        THEME.text_color,
    )
}

/// Where the server serves an uploaded picture.
pub fn asset_url(asset_id: &AssetId) -> String {
    format!("/assets/{}", asset_id.as_str())
}

/// Draws one slide's content into the given rectangle.
#[component]
pub fn SlideView(slide: Slide, content: Rect, font_px: u32, wrap: bool) -> impl IntoView {
    let lines = match slide.body {
        SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => lines,
        SlideBody::Image { asset_id } => return image_view(&asset_id, content).into_any(),
    };
    view! {
        <div class=lines_class(wrap) style=text_box_style(content, font_px)>
            {lines.into_iter().map(|line| view! { <div class="sursum-line">{line}</div> }).collect_view()}
        </div>
    }
    .into_any()
}

/// A picture inside the content rectangle (so it also avoids exclusion zones), always whole: a
/// poster cropped at the edges would lose its text.
fn image_view(asset_id: &AssetId, content: Rect) -> impl IntoView {
    let style = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;object-fit:contain;",
        content.x, content.y, content.width, content.height
    );
    view! { <img class="sursum-image" src=asset_url(asset_id) style=style alt="" /> }
}
