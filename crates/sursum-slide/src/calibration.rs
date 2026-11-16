// SPDX-License-Identifier: GPL-3.0-or-later
//! The calibration test card (ADR 0008): a numbered grid over the whole frame, the safe area,
//! the exclusion zones and the rectangle left for text. The screen shows it only while an
//! operator calibrates, outside services; the remote's calibration panel draws the same card, so
//! what the operator drags on the tablet lines up with what the person in the nave sees.

use leptos::prelude::*;
use sursum_protocol::{ExclusionZone, MaskMode, Rect, SCENE_HEIGHT, SCENE_WIDTH, ScreenProfile};

/// Size of one grid cell in scene pixels (16 × 9 cells).
pub const GRID_CELL: u32 = 120;

/// The test card for `profile`; `content` is the text rectangle the server computed from it.
/// Place it inside a [`crate::Stage`]; it covers the whole scene.
#[component]
pub fn CalibrationPattern(
    #[prop(into)] profile: Signal<ScreenProfile>,
    #[prop(into)] content: Signal<Rect>,
) -> impl IntoView {
    let safe_area = move || {
        let insets = profile.with(|profile| profile.safe_area);
        let rect = Rect::new(
            insets.left,
            insets.top,
            SCENE_WIDTH.saturating_sub(insets.left + insets.right),
            SCENE_HEIGHT.saturating_sub(insets.top + insets.bottom),
        );
        outline("sursum-calibration-safe", rect)
    };
    let zones = move || profile.get().exclusions.into_iter().map(zone_view).collect_view();
    let text_area = move || outline("sursum-calibration-content", content.get());
    view! {
        <div class="sursum-calibration" data-calibration="on">
            {grid_labels()}
            {safe_area}
            {text_area}
            {zones}
        </div>
    }
}

/// Column letters along the top and row numbers along the left, so the person in the nave can
/// say "the pillar covers columns D to E".
fn grid_labels() -> impl IntoView {
    let columns = (0..SCENE_WIDTH / GRID_CELL).map(|column| {
        let letter = char::from(b'A' + u8::try_from(column).unwrap_or(0));
        label(column * GRID_CELL + GRID_CELL / 2 - 10, 6, letter.to_string())
    });
    let rows = (0..SCENE_HEIGHT / GRID_CELL)
        .map(|row| label(8, row * GRID_CELL + GRID_CELL / 2 - 16, (row + 1).to_string()));
    columns.chain(rows).collect_view()
}

fn label(x: u32, y: u32, text: String) -> impl IntoView {
    let style = format!("left:{x}px;top:{y}px;");
    view! {
        <span class="sursum-calibration-label" style=style>
            {text}
        </span>
    }
}

fn zone_view(zone: ExclusionZone) -> impl IntoView {
    let class = match zone.mask {
        MaskMode::Black => "sursum-calibration-zone",
        MaskMode::None => "sursum-calibration-zone unmasked",
    };
    view! {
        <div class=class style=rect_style(zone.rect)>
            <span class="sursum-calibration-name">{zone.name}</span>
        </div>
    }
}

fn outline(class: &'static str, rect: Rect) -> impl IntoView {
    view! { <div class=class style=rect_style(rect)></div> }
}

/// Absolute position and size of a scene rectangle, as inline CSS.
pub fn rect_style(rect: Rect) -> String {
    format!("left:{}px;top:{}px;width:{}px;height:{}px;", rect.x, rect.y, rect.width, rect.height)
}
