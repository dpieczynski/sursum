// SPDX-License-Identifier: GPL-3.0-or-later
//! The audience screen: two content layers (A/B) that crossfade, black masks over exclusion
//! zones, and the black/logo overlay on top.
//!
//! The next slide is drawn into the hidden layer; only when its font size is known (fonts
//! loaded) does it fade in over the old one. Both layers have opaque backgrounds, so the fade
//! never dims the screen or shows two texts at once. A single async loop performs every
//! transition, so the layers are changed in exactly one place.

use futures_util::StreamExt;
use leptos::html::Div;
use leptos::prelude::*;
use sursum_protocol::{DeckId, Overlay, Rect, ScreenLayout, ScreenState, Slide, SlideBody, SlideKey};
use wasm_bindgen::JsCast;

use crate::calibration::{CalibrationPattern, rect_style};
use crate::dom;
use crate::fit::{Fit, fit_deck};
use crate::slide_view::{SlideView, asset_url};
use crate::stage::Stage;

/// What one content layer shows.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    deck_id: DeckId,
    slide: Slide,
    layout: ScreenLayout,
    fit: Fit,
}

impl Shown {
    fn same_as(&self, deck_id: &DeckId, key: &SlideKey, layout: &ScreenLayout) -> bool {
        &self.deck_id == deck_id && &self.slide.key == key && &self.layout == layout
    }
}

/// The screen. Feed it the latest snapshot; it never shows anything but content, black or logo.
#[component]
pub fn LayeredStage(#[prop(into)] state: Signal<Option<ScreenState>>) -> impl IntoView {
    let measurer = NodeRef::<Div>::new();
    let layer_refs = [NodeRef::<Div>::new(), NodeRef::<Div>::new()];
    let layers = [RwSignal::new(None::<Shown>), RwSignal::new(None::<Shown>)];
    let front = RwSignal::new(0_usize);
    let transitions = RwSignal::new(0_u32);
    let running = StoredValue::new_local(None::<web_sys::Animation>);
    let snapshots = StoredValue::new(0_u64);
    let (wake, mut woken) = futures_channel::mpsc::unbounded::<()>();

    // Every new snapshot wakes the transition loop and cuts a running crossfade short.
    Effect::new(move |_| {
        state.track();
        snapshots.update_value(|count| *count += 1);
        running.with_value(|animation| {
            if let Some(animation) = animation {
                let _ = animation.finish();
            }
        });
        let _ = wake.unbounded_send(());
    });

    let parts = Parts { measurer, layer_refs, layers, front, transitions, running, snapshots };
    leptos::task::spawn_local(async move {
        while woken.next().await.is_some() {
            // Only the newest snapshot matters.
            while woken.try_recv().is_ok() {}
            if let Some(target) = state.get_untracked() {
                parts.transition_to(target).await;
            }
        }
    });

    let shown = move || layers[front.get()].get();
    let overlay = move || state.with(|state| state.as_ref().map(|state| state.overlay).unwrap_or_default());
    // How many snapshots have arrived (counting stops at two) and how long the overlay fades for
    // the current one. The first snapshot after the page loads (a kiosk restart, a boot) shows its
    // overlay at once: fading it in would let the live slide under black or the logo show for a
    // moment. The duration changes in the same render as the overlay, so no fade ever starts.
    let overlay_fade = Memo::new(move |previous: Option<&(u8, u32)>| {
        let seen = previous.map_or(0, |(seen, _)| *seen);
        match state.with(|state| state.as_ref().map(|state| state.transition_ms)) {
            None => (seen, 0),
            Some(_) if seen == 0 => (1, 0),
            Some(ms) => (2, ms),
        }
    });
    let overlay_ms = move || overlay_fade.get().1;
    let masks = move || {
        state.with(|state| state.as_ref().map(|state| state.layout.masks.clone()).unwrap_or_default())
    };

    // Test hooks for the WebDriver suite (ADR 0015): what is shown and how often it changed.
    let deck_id_attr = move || shown().map(|shown| shown.deck_id.0).unwrap_or_default();
    let slide_key_attr = move || shown().map(|shown| shown.slide.key.0).unwrap_or_default();
    let font_px_attr = move || shown().map(|shown| shown.fit.font_px.to_string()).unwrap_or_default();
    let transitions_attr = move || transitions.get().to_string();
    let overlay_attr = move || overlay_name(overlay());
    let mask_views = move || masks().into_iter().map(mask_view).collect_view();
    let calibration = move || state.with(|state| state.as_ref().and_then(|state| state.calibration.clone()));
    let content =
        move || state.with(|state| state.as_ref().map_or(Rect::SCENE, |state| state.layout.content));
    let logo = move || state.with(|state| state.as_ref().and_then(|state| state.logo.clone()));
    // Decode a new logo right away, so the logo overlay never fades in half-loaded.
    Effect::new(move |_| {
        if let Some(asset_id) = logo() {
            leptos::task::spawn_local(async move { dom::decode_image(&asset_url(&asset_id)).await });
        }
    });
    // The cover last raised: the logo stays while the overlay fades out after "logo", but black
    // stays plain black even with a logo configured.
    let cover = Memo::new(move |previous: Option<&Overlay>| match overlay() {
        Overlay::None => previous.copied().unwrap_or_default(),
        raised => raised,
    });
    let logo_view = move || {
        logo().filter(|_| cover.get() == Overlay::Logo).map(|asset_id| {
            view! {
                <div class="sursum-logo-box" style=rect_style(content())>
                    <img class="sursum-logo" src=asset_url(&asset_id) alt="" />
                </div>
            }
        })
    };
    let pattern = move || {
        calibration().map(|profile| view! { <CalibrationPattern profile content=Signal::derive(content) /> })
    };
    let overlay_style = move || format!("z-index:10;transition-duration:{}ms;", overlay_ms());

    view! {
        <Stage class="sursum-screen">
            <div
                class="sursum-layer"
                data-deck-id=deck_id_attr
                data-slide-key=slide_key_attr
                data-font-px=font_px_attr
                data-transitions=transitions_attr
                data-overlay=overlay_attr
            >
                {layer_view(layers[0], layer_refs[0])}
                {layer_view(layers[1], layer_refs[1])}
                <div class="sursum-layer sursum-measure" node_ref=measurer></div>
                {mask_views}
                <div
                    class="sursum-overlay"
                    class:visible=move || overlay() != Overlay::None
                    style=overlay_style
                >
                    {logo_view}
                </div>
                {pattern}
            </div>
        </Stage>
    }
}

/// Signals and element references the transition loop works with.
#[derive(Clone, Copy)]
struct Parts {
    measurer: NodeRef<Div>,
    layer_refs: [NodeRef<Div>; 2],
    layers: [RwSignal<Option<Shown>>; 2],
    front: RwSignal<usize>,
    transitions: RwSignal<u32>,
    running: StoredValue<Option<web_sys::Animation>, LocalStorage>,
    /// Counts snapshots, so a transition can tell that a newer one arrived while it waited.
    snapshots: StoredValue<u64>,
}

impl Parts {
    /// Brings the screen to `target`. Does nothing if that slide is already shown (e.g. after a
    /// reconnect), so reconnecting never causes a visible transition.
    async fn transition_to(self, target: ScreenState) {
        let Some(live) = target.live else { return };
        let Some(slide) = crate::shown::shown_slide(&live) else { return };
        let current = self.front.get_untracked();
        let unchanged = self.layers[current].with_untracked(|shown| {
            shown.as_ref().is_some_and(|shown| shown.same_as(&live.deck.deck_id, &slide.key, &target.layout))
        });
        if unchanged {
            return;
        }
        let Some(measurer) = self.measurer.get_untracked() else { return };
        let measurer: web_sys::HtmlElement = measurer.unchecked_into();
        // A newer snapshot while fitting or decoding makes this one stale: give up before it
        // shows (the loop goes on with the newest), instead of fading in a slide already passed.
        let snapshot = self.snapshots.get_value();
        let superseded = || self.snapshots.get_value() != snapshot;
        let fit = fit_deck(&live.deck, target.layout.content, &measurer).await;
        if let SlideBody::Image { asset_id, .. } = &slide.body {
            dom::decode_image(&asset_url(asset_id)).await;
        }
        if superseded() {
            return;
        }

        let incoming = 1 - current;
        self.layers[incoming].set(Some(Shown {
            deck_id: live.deck.deck_id.clone(),
            slide,
            layout: target.layout.clone(),
            fit,
        }));
        // Let Leptos render the incoming layer before showing it.
        dom::next_frame().await;
        let (Some(incoming_element), Some(outgoing_element)) =
            (self.layer_refs[incoming].get_untracked(), self.layer_refs[current].get_untracked())
        else {
            return;
        };
        set_layer_style(&incoming_element, 1, 2);
        if target.transition_ms > 0
            && let Some(animation) = dom::fade_in(&incoming_element, target.transition_ms)
        {
            self.running.set_value(Some(animation.clone()));
            dom::finished(&animation).await;
            self.running.set_value(None);
        }
        set_layer_style(&incoming_element, 1, 1);
        set_layer_style(&outgoing_element, 0, 0);
        self.front.set(incoming);
        self.transitions.update(|count| *count += 1);

        // Fit the next item in advance so its first slide appears without delay.
        if let Some(upcoming) = target.upcoming {
            let _ = fit_deck(&upcoming, target.layout.content, &measurer).await;
        }
    }
}

/// One content layer; its opacity and stacking are set by the transition loop, not by Leptos.
/// Leptos must never set its `style` attribute: replacing it wiped the loop's opacity, so a new
/// slide showed at full brightness for a frame before its fade (seen on the Pi at the first song
/// after boot). Opaque black and hidden until the loop shows it (CSS).
fn layer_view(shown: RwSignal<Option<Shown>>, node_ref: NodeRef<Div>) -> impl IntoView {
    view! {
        <div class="sursum-layer sursum-opaque sursum-content" node_ref=node_ref>
            {move || {
                shown
                    .get()
                    .map(|shown| {
                        view! {
                            <SlideView
                                slide=shown.slide
                                content=shown.layout.content
                                font_px=shown.fit.font_px
                                wrap=shown.fit.wrap
                            />
                        }
                    })
            }}
        </div>
    }
}

/// Sets opacity and stacking order directly on a layer element.
fn set_layer_style(element: &web_sys::HtmlDivElement, opacity: u8, z_index: u8) {
    let style = web_sys::HtmlElement::style(element);
    let _ = style.set_property("opacity", &opacity.to_string());
    let _ = style.set_property("z-index", &z_index.to_string());
}

fn overlay_name(overlay: Overlay) -> &'static str {
    match overlay {
        Overlay::None => "none",
        Overlay::Black => "black",
        Overlay::Logo => "logo",
    }
}

/// A black rectangle over an exclusion zone (above content, below the overlay).
fn mask_view(rect: sursum_protocol::Rect) -> impl IntoView {
    let style = format!(
        "left:{}px;top:{}px;width:{}px;height:{}px;z-index:5;",
        rect.x, rect.y, rect.width, rect.height
    );
    view! { <div class="sursum-mask" style=style></div> }
}
