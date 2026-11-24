// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact numbers for the calibration panel: one row per exclusion zone, and the safe area.
//! Every edit goes straight to the server through [`ProfileEditor`].
//!
//! The fields are capped by each other so a zone always stays on the stage: moving a zone keeps
//! its size (the offset stops where the zone touches the edge), resizing keeps its position (the
//! size stops at the edge). The server cuts whatever still arrives to the stage, too.

use leptos::prelude::*;
use sursum_protocol::{ExclusionZone, Insets, MaskMode, Rect, SCENE_HEIGHT, SCENE_WIDTH};

use crate::calibration::{ProfileEditor, new_zone};
use crate::messages::{Msg, pl};

/// Where a new zone starts: a vertical strip in the middle (drag it into place afterwards).
const NEW_ZONE: Rect = Rect::new(840, 0, 240, 1080);

/// One row per zone; the selected row is the one a drag on the test card redraws.
#[component]
pub fn ZoneList(editor: ProfileEditor, selected: RwSignal<usize>) -> impl IntoView {
    let rows = move || {
        let zones = editor.profile.get().exclusions;
        zones
            .into_iter()
            .enumerate()
            .map(|(index, zone)| zone_row(index, zone, editor, selected))
            .collect_view()
    };
    let add = move |_| {
        editor.change(|profile| profile.exclusions.push(new_zone(profile.exclusions.len(), NEW_ZONE)));
        selected.set(editor.profile.get_untracked().exclusions.len());
    };
    view! {
        <div class="calibration-zones">
            {rows} <button class="btn btn--sm" on:click=add>
                {pl(Msg::AddZone)}
            </button>
        </div>
    }
}

fn zone_row(
    index: usize,
    zone: ExclusionZone,
    editor: ProfileEditor,
    selected: RwSignal<usize>,
) -> impl IntoView {
    let edit = move |change: fn(&mut ExclusionZone, &str), value: String| {
        editor.change(|profile| {
            if let Some(zone) = profile.exclusions.get_mut(index) {
                change(zone, &value);
            }
        });
    };
    let remove = move |_| {
        editor.change(|profile| {
            if index < profile.exclusions.len() {
                profile.exclusions.remove(index);
            }
        });
        selected.set(0);
    };
    let masked = zone.mask == MaskMode::Black;
    let toggle_mask = move |_| {
        editor.change(|profile| {
            if let Some(zone) = profile.exclusions.get_mut(index) {
                zone.mask = if zone.mask == MaskMode::Black { MaskMode::None } else { MaskMode::Black };
            }
        });
    };
    let rect = zone.rect;
    view! {
        <div class="calibration-row" class:selected=move || selected.get() == index>
            <input
                type="radio"
                name="zone"
                title=pl(Msg::DrawThisZone)
                prop:checked=move || selected.get() == index
                on:change=move |_| selected.set(index)
            />
            <input
                class="zone-name"
                type="text"
                prop:value=zone.name
                on:change=move |event| edit(
                    |zone, value| zone.name = value.to_owned(),
                    event_target_value(&event),
                )
            />
            {number_field("x", rect.x, max_offset(SCENE_WIDTH, rect.width), move |value| edit(set_x, value))}
            {number_field(
                "y",
                rect.y,
                max_offset(SCENE_HEIGHT, rect.height),
                move |value| edit(set_y, value),
            )}
            {number_field(
                pl(Msg::Width),
                rect.width,
                SCENE_WIDTH - rect.x.min(SCENE_WIDTH),
                move |value| edit(set_width, value),
            )}
            {number_field(
                pl(Msg::Height),
                rect.height,
                SCENE_HEIGHT - rect.y.min(SCENE_HEIGHT),
                move |value| edit(set_height, value),
            )}
            <label class="zone-mask">
                <input type="checkbox" prop:checked=masked on:change=toggle_mask />
                {pl(Msg::MaskZone)}
            </label>
            <button class="btn btn--sm" on:click=remove>
                {pl(Msg::RemoveZone)}
            </button>
        </div>
    }
}

/// The largest offset at which a zone of `size` still fits on a stage side of `side`.
fn max_offset(side: u32, size: u32) -> u32 {
    side.saturating_sub(size)
}

fn set_x(zone: &mut ExclusionZone, value: &str) {
    zone.rect.x = parse_px(value, zone.rect.x).min(max_offset(SCENE_WIDTH, zone.rect.width));
}
fn set_y(zone: &mut ExclusionZone, value: &str) {
    zone.rect.y = parse_px(value, zone.rect.y).min(max_offset(SCENE_HEIGHT, zone.rect.height));
}
fn set_width(zone: &mut ExclusionZone, value: &str) {
    zone.rect.width = parse_px(value, zone.rect.width).min(max_offset(SCENE_WIDTH, zone.rect.x));
}
fn set_height(zone: &mut ExclusionZone, value: &str) {
    zone.rect.height = parse_px(value, zone.rect.height).min(max_offset(SCENE_HEIGHT, zone.rect.y));
}

/// The largest margin that still leaves one pixel next to the opposite margin.
fn max_margin(side: u32, opposite: u32) -> u32 {
    side.saturating_sub(opposite).saturating_sub(1)
}

/// The safe area (overscan margins) in scene pixels.
#[component]
pub fn SafeAreaForm(editor: ProfileEditor) -> impl IntoView {
    let insets = move || editor.profile.get().safe_area;
    let edit = move |change: fn(&mut Insets, u32), value: String| {
        editor.change(|profile| {
            if let Ok(value) = value.trim().parse() {
                change(&mut profile.safe_area, value);
            }
        });
    };
    let fields = move || {
        let insets = insets();
        let top = max_margin(SCENE_HEIGHT, insets.bottom);
        let bottom = max_margin(SCENE_HEIGHT, insets.top);
        let left = max_margin(SCENE_WIDTH, insets.right);
        let right = max_margin(SCENE_WIDTH, insets.left);
        view! {
            {number_field(pl(Msg::MarginTop), insets.top, top, move |value| edit(set_top, value))}
            {number_field(pl(Msg::MarginBottom), insets.bottom, bottom, move |value| edit(set_bottom, value))}
            {number_field(pl(Msg::MarginLeft), insets.left, left, move |value| edit(set_left, value))}
            {number_field(pl(Msg::MarginRight), insets.right, right, move |value| edit(set_right, value))}
        }
    };
    view! {
        <div class="calibration-row">
            <span class="zone-name">{pl(Msg::SafeArea)}</span>
            {fields}
        </div>
    }
}

fn set_top(insets: &mut Insets, value: u32) {
    insets.top = value.min(max_margin(SCENE_HEIGHT, insets.bottom));
}
fn set_bottom(insets: &mut Insets, value: u32) {
    insets.bottom = value.min(max_margin(SCENE_HEIGHT, insets.top));
}
fn set_left(insets: &mut Insets, value: u32) {
    insets.left = value.min(max_margin(SCENE_WIDTH, insets.right));
}
fn set_right(insets: &mut Insets, value: u32) {
    insets.right = value.min(max_margin(SCENE_WIDTH, insets.left));
}

/// A labelled number input in scene pixels up to `max`; `on_change` gets the raw text when it is
/// committed.
fn number_field(
    label: &'static str,
    value: u32,
    max: u32,
    on_change: impl Fn(String) + 'static,
) -> impl IntoView {
    view! {
        <label class="number-field">
            <span>{label}</span>
            <input
                type="number"
                inputmode="numeric"
                min="0"
                max=max.to_string()
                prop:value=value.to_string()
                on:change=move |event| on_change(event_target_value(&event))
            />
            <span class="unit">{pl(Msg::Px)}</span>
        </label>
    }
}

/// A pixel value from a form field; anything that is not a number keeps the old value.
fn parse_px(text: &str, old: u32) -> u32 {
    text.trim().parse().unwrap_or(old)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_stays_on_the_stage_whichever_field_is_typed() {
        let mut zone = ExclusionZone {
            name: "Strefa 1".into(),
            rect: Rect::new(100, 0, 400, 1080),
            mask: MaskMode::Black,
        };
        set_x(&mut zone, "1800");
        assert_eq!(zone.rect, Rect::new(1520, 0, 400, 1080), "moving keeps the size");
        set_width(&mut zone, "5000");
        assert_eq!(zone.rect.width, 400, "already touches the edge");
        set_x(&mut zone, "0");
        set_width(&mut zone, "5000");
        assert_eq!(zone.rect, Rect::new(0, 0, 1920, 1080), "resizing keeps the position");
        set_y(&mut zone, "10");
        assert_eq!(zone.rect.y, 0, "a full-height zone cannot move down");
        let mut insets = Insets::uniform(48);
        set_left(&mut insets, "9999".parse().unwrap());
        assert_eq!(insets.left, 1920 - 48 - 1, "one pixel stays between the margins");
    }

    #[test]
    fn bad_numbers_keep_the_old_value() {
        assert_eq!(parse_px(" 120 ", 5), 120);
        assert_eq!(parse_px("abc", 5), 5);
        assert_eq!(parse_px("-3", 5), 5);
    }
}
