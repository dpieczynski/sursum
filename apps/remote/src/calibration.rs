// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen calibration (ADR 0008): mark the parts of the frame that text must avoid. Outside services only.
//!
//! The panel holds no profile of its own: every change is sent to the server at once
//! (`SetScreenProfile`), the server validates it and answers with a new snapshot, and both the
//! screen and this panel draw the same test card from that snapshot. Dragging on the card draws
//! the selected zone; the form below allows exact numbers.

use leptos::prelude::*;
use sursum_protocol::{
    Command, ExclusionZone, MaskMode, Rect, RemoteState, SCENE_HEIGHT, SCENE_WIDTH, ScreenProfile,
};
use sursum_slide::{CalibrationPattern, Stage};

use crate::Commands;
use crate::calibration_form::{SafeAreaForm, ZoneList};
use crate::messages::{Msg, pl};

/// True while the calibration panel is open (shared through the Leptos context).
#[derive(Clone, Copy)]
pub struct Calibrating(pub RwSignal<bool>);

/// The calibration panel; `open` is cleared when it closes.
#[component]
pub fn CalibrationPanel(
    open: RwSignal<bool>,
    state: RwSignal<Option<RemoteState>>,
    commands: Commands,
) -> impl IntoView {
    commands.send(Command::Calibrate { on: true });
    let profile = Signal::derive(move || {
        state.with(|state| state.as_ref().map(|state| state.screen_profile.clone()).unwrap_or_default())
    });
    let content = Signal::derive(move || {
        state.with(|state| state.as_ref().map_or(Rect::SCENE, |state| state.program.layout.content))
    });
    let selected = RwSignal::new(0_usize);
    let editor = ProfileEditor { profile, commands };
    let close = move |_| {
        commands.send(Command::Calibrate { on: false });
        open.set(false);
    };
    let draw = move |rect: Rect| {
        let index = selected.get_untracked();
        editor.change(|profile| match profile.exclusions.get_mut(index) {
            Some(zone) => zone.rect = rect,
            None => profile.exclusions.push(new_zone(profile.exclusions.len(), rect)),
        });
    };
    view! {
        <div class="calibration">
            <header class="calibration-header">
                <h2>{pl(Msg::CalibrationTitle)}</h2>
                <button class="close" on:click=close>
                    {pl(Msg::Close)}
                </button>
            </header>
            <p class="editor-help">{pl(Msg::CalibrationHelp)}</p>
            <DrawingFrame profile content on_draw=draw />
            <ZoneList editor selected />
            <SafeAreaForm editor />
        </div>
    }
}

/// Changes the server's profile: copy, modify, send. The server validates and broadcasts it.
#[derive(Clone, Copy)]
pub struct ProfileEditor {
    pub profile: Signal<ScreenProfile>,
    commands: Commands,
}

impl ProfileEditor {
    pub fn change(self, edit: impl FnOnce(&mut ScreenProfile)) {
        let mut next = self.profile.get_untracked();
        edit(&mut next);
        self.commands.send(Command::SetScreenProfile { profile: next });
    }
}

/// A new zone, named "Strefa 1", "Strefa 2", …
pub fn new_zone(existing: usize, rect: Rect) -> ExclusionZone {
    ExclusionZone { name: format!("{} {}", pl(Msg::Zone), existing + 1), rect, mask: MaskMode::Black }
}

/// The test card with a transparent layer on top that turns a drag into a scene rectangle.
#[component]
fn DrawingFrame(
    profile: Signal<ScreenProfile>,
    content: Signal<Rect>,
    on_draw: impl Fn(Rect) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let surface = NodeRef::<leptos::html::Div>::new();
    let start = RwSignal::new(None::<(u32, u32)>);
    let current = RwSignal::new(None::<Rect>);
    let to_scene = move |event: &leptos::ev::PointerEvent| {
        let element = surface.get_untracked()?;
        scene_point(&element.get_bounding_client_rect(), event.client_x(), event.client_y())
    };
    let down = move |event: leptos::ev::PointerEvent| {
        if let Some(point) = to_scene(&event) {
            if let Some(element) = surface.get_untracked() {
                let _ = element.set_pointer_capture(event.pointer_id());
            }
            start.set(Some(point));
            current.set(None);
        }
    };
    let moved = move |event: leptos::ev::PointerEvent| {
        if let (Some(from), Some(to)) = (start.get_untracked(), to_scene(&event)) {
            current.set(Some(span(from, to)));
        }
    };
    let up = move |_| {
        start.set(None);
        if let Some(rect) = current.get_untracked().filter(|rect| rect.width >= 8 && rect.height >= 8) {
            on_draw(rect);
        }
        current.set(None);
    };
    let dragged = move || {
        current.get().map(|rect| {
            view! { <div class="calibration-drag" style=sursum_slide::calibration::rect_style(rect)></div> }
        })
    };
    view! {
        <div class="calibration-frame">
            <Stage>
                <CalibrationPattern profile content />
                {dragged}
                <div
                    class="calibration-surface"
                    node_ref=surface
                    on:pointerdown=down
                    on:pointermove=moved
                    on:pointerup=up
                    on:pointercancel=move |_| start.set(None)
                ></div>
            </Stage>
        </div>
    }
}

/// A browser point inside the (scaled) stage → scene pixels, clamped to the frame.
fn scene_point(bounds: &web_sys::DomRect, client_x: i32, client_y: i32) -> Option<(u32, u32)> {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    let scale = |offset: f64, size: f64, scene: u32| {
        let value = (offset / size * f64::from(scene)).round().clamp(0.0, f64::from(scene));
        // In range after the clamp, so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixels = value as u32;
        pixels
    };
    Some((
        scale(f64::from(client_x) - bounds.left(), bounds.width(), SCENE_WIDTH),
        scale(f64::from(client_y) - bounds.top(), bounds.height(), SCENE_HEIGHT),
    ))
}

/// The rectangle spanned by two corners, in any drag direction.
fn span((x1, y1): (u32, u32), (x2, y2): (u32, u32)) -> Rect {
    Rect::new(x1.min(x2), y1.min(y2), x1.abs_diff(x2), y1.abs_diff(y2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drags_in_any_direction_give_the_same_rectangle() {
        assert_eq!(span((100, 50), (300, 450)), Rect::new(100, 50, 200, 400));
        assert_eq!(span((300, 450), (100, 50)), Rect::new(100, 50, 200, 400));
    }
}
