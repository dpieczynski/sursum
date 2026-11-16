// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact numbers for the calibration panel: one row per exclusion zone, and the safe area.
//! Every edit goes straight to the server through [`ProfileEditor`].

use leptos::prelude::*;
use sursum_protocol::{ExclusionZone, Insets, MaskMode, Rect};

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
            {rows} <button class="import-button" on:click=add>
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
            {number_field("x", rect.x, move |value| edit(set_x, value))}
            {number_field("y", rect.y, move |value| edit(set_y, value))}
            {number_field(pl(Msg::Width), rect.width, move |value| edit(set_width, value))}
            {number_field(pl(Msg::Height), rect.height, move |value| edit(set_height, value))}
            <label class="zone-mask">
                <input type="checkbox" prop:checked=masked on:change=toggle_mask />
                {pl(Msg::MaskZone)}
            </label>
            <button class="row-button" on:click=remove>
                {pl(Msg::RemoveZone)}
            </button>
        </div>
    }
}

fn set_x(zone: &mut ExclusionZone, value: &str) {
    zone.rect.x = parse_px(value, zone.rect.x);
}
fn set_y(zone: &mut ExclusionZone, value: &str) {
    zone.rect.y = parse_px(value, zone.rect.y);
}
fn set_width(zone: &mut ExclusionZone, value: &str) {
    zone.rect.width = parse_px(value, zone.rect.width);
}
fn set_height(zone: &mut ExclusionZone, value: &str) {
    zone.rect.height = parse_px(value, zone.rect.height);
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
        view! {
            {number_field(pl(Msg::MarginTop), insets.top, move |value| edit(|i, v| i.top = v, value))}
            {number_field(
                pl(Msg::MarginBottom),
                insets.bottom,
                move |value| edit(|i, v| i.bottom = v, value),
            )}
            {number_field(pl(Msg::MarginLeft), insets.left, move |value| edit(|i, v| i.left = v, value))}
            {number_field(pl(Msg::MarginRight), insets.right, move |value| edit(|i, v| i.right = v, value))}
        }
    };
    view! {
        <div class="calibration-row">
            <span class="zone-name">{pl(Msg::SafeArea)}</span>
            {fields}
        </div>
    }
}

/// A labelled number input in scene pixels; `on_change` gets the raw text when it is committed.
fn number_field(label: &'static str, value: u32, on_change: impl Fn(String) + 'static) -> impl IntoView {
    view! {
        <label class="number-field">
            <span>{label}</span>
            <input
                type="number"
                inputmode="numeric"
                min="0"
                max="1920"
                prop:value=value.to_string()
                on:change=move |event| on_change(event_target_value(&event))
            />
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
    fn bad_numbers_keep_the_old_value() {
        assert_eq!(parse_px(" 120 ", 5), 120);
        assert_eq!(parse_px("abc", 5), 5);
        assert_eq!(parse_px("-3", 5), 5);
    }
}
