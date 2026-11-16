// SPDX-License-Identifier: GPL-3.0-or-later
//! Small wrappers around browser APIs used by the screen. Only standard APIs that WebKit (WPE)
//! also supports are allowed here (ADR 0015).

use std::time::Duration;

use sursum_protocol::Theme;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// Waits until the theme's font is loaded, so measurements use the real metrics.
pub async fn fonts_ready(theme: &Theme) {
    let Some(fonts) = web_sys::window().and_then(|window| window.document()).map(|document| document.fonts())
    else {
        return;
    };
    let spec = format!("{} 100px \"{}\"", theme.font_weight, theme.font_family);
    // Polish letters live in the latin-ext subset; load it explicitly too.
    let _ = JsFuture::from(fonts.load_with_text(&spec, "AĄŁżź")).await;
    if let Ok(promise) = fonts.ready() {
        let _ = JsFuture::from(promise).await;
    }
}

/// Loads and decodes a picture, so it appears complete on the first frame it is shown
/// (`img.decode()`, allowed by the screen API contract, ADR 0015). Errors are ignored: a broken
/// picture shows as nothing, never as an error on the screen.
pub async fn decode_image(url: &str) {
    let Ok(image) = web_sys::HtmlImageElement::new() else { return };
    image.set_src(url);
    let _ = JsFuture::from(image.decode()).await;
}

/// Fades an element in with the Web Animations API and returns the running animation.
pub fn fade_in(element: &web_sys::Element, duration_ms: u32) -> Option<web_sys::Animation> {
    let frame = |opacity: &str| {
        let object = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&object, &"opacity".into(), &opacity.into());
        object
    };
    let keyframes = js_sys::Array::of2(&frame("0"), &frame("1"));
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"duration".into(), &JsValue::from(duration_ms));
    // No fill: the caller sets the final opacity as a style before the animation starts, so the
    // element keeps it after the animation ends without a lingering animation effect.
    let _ = js_sys::Reflect::set(&options, &"fill".into(), &"none".into());
    let _ = js_sys::Reflect::set(&options, &"easing".into(), &"linear".into());
    let animate: js_sys::Function = js_sys::Reflect::get(element, &"animate".into()).ok()?.dyn_into().ok()?;
    animate.call2(element, &keyframes, &options).ok()?.dyn_into().ok()
}

/// Waits for an animation to end (normally or through `finish()`).
pub async fn finished(animation: &web_sys::Animation) {
    if let Ok(promise) = animation.finished() {
        let _ = JsFuture::from(promise).await;
    }
}

/// Lets the browser apply pending DOM updates before continuing.
pub async fn next_frame() {
    gloo_timers::future::sleep(Duration::from_millis(16)).await;
}
