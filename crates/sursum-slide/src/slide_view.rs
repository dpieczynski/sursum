// SPDX-License-Identifier: GPL-3.0-or-later
//! One slide, drawn inside a 1920×1080 stage.

use leptos::prelude::*;
use sursum_protocol::{AssetId, Background, ImageFit, Rect, Slide, SlideBody, TextAlign, Theme};

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
    format!("background:{};", background_value(background))
}

/// The value of the `background` property alone, for an element whose other inline styles
/// (opacity, stacking) are set elsewhere and must not be replaced.
pub fn background_value(background: &Background) -> String {
    match background {
        Background::Color { css } => css.clone(),
    }
}

/// Where the server serves an uploaded picture.
pub fn asset_url(asset_id: &AssetId) -> String {
    format!("/assets/{}", asset_id.as_str())
}

/// Draws one slide's content into the given rectangle.
#[component]
pub fn SlideView(slide: Slide, theme: Theme, content: Rect, font_px: u32, wrap: bool) -> impl IntoView {
    let lines = match slide.body {
        SlideBody::Lyrics { lines } | SlideBody::Text { lines, .. } => lines,
        SlideBody::Image { asset_id, fit } => return image_view(&asset_id, fit, content).into_any(),
    };
    view! {
        <div class=lines_class(&theme, wrap) style=text_box_style(&theme, content, font_px)>
            {lines.into_iter().map(|line| view! { <div class="sursum-line">{line}</div> }).collect_view()}
        </div>
    }
    .into_any()
}

/// A picture inside the content rectangle (so it also avoids exclusion zones).
fn image_view(asset_id: &AssetId, fit: ImageFit, content: Rect) -> impl IntoView {
    let object_fit = match fit {
        ImageFit::Contain => "contain",
        ImageFit::Cover => "cover",
    };
    let style = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;object-fit:{object_fit};",
        content.x, content.y, content.width, content.height
    );
    view! { <img class="sursum-image" src=asset_url(asset_id) style=style alt="" /> }
}
