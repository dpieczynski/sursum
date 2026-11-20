// SPDX-License-Identifier: GPL-3.0-or-later
//! A static preview of one slide for the remote: the same stage, fitter and slide component as
//! the screen, without transitions.

use leptos::html::Div;
use leptos::prelude::*;
use sursum_protocol::{LiveSlide, ScreenLayout};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::fit::{Fit, fit_deck};
use crate::slide_view::SlideView;
use crate::stage::Stage;

/// What a preview shows.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewData {
    pub live: LiveSlide,
    pub layout: ScreenLayout,
}

/// A scaled preview. `dimmed` greys the content out (e.g. while the screen is black).
#[component]
pub fn SlidePreview(
    #[prop(into)] data: Signal<Option<PreviewData>>,
    #[prop(into, optional)] dimmed: Signal<bool>,
    #[prop(into, optional)] class: String,
) -> impl IntoView {
    let measurer = NodeRef::<Div>::new();
    let fit = RwSignal::new(None::<(PreviewData, Fit)>);
    // A preview hidden by the layout (e.g. "next slide" before a tablet is turned upright) cannot
    // measure and gets the smallest size; counting the moments it is laid out again makes the
    // fit below run once more then, instead of keeping the tiny text until the slide changes.
    let laid_out = RwSignal::new(0_u32);
    Effect::new(move |_| {
        let Some(element) = measurer.get() else { return };
        when_laid_out(element.unchecked_into(), move || laid_out.update(|count| *count += 1));
    });

    Effect::new(move |_| {
        laid_out.track();
        let Some(current) = data.get() else {
            fit.set(None);
            return;
        };
        let Some(element) = measurer.get() else { return };
        let element: web_sys::HtmlElement = element.unchecked_into();
        leptos::task::spawn_local(async move {
            let result = fit_deck(&current.live.deck, current.layout.content, &element).await;
            // Ignore results for data that changed while measuring.
            if data.get_untracked().as_ref() == Some(&current) {
                fit.set(Some((current, result)));
            }
        });
    });

    let content = move || {
        fit.get().and_then(|(data, fit)| {
            let slide = crate::shown::shown_slide(&data.live)?;
            let masks = data.layout.masks.clone();
            Some(view! {
                <div class="sursum-layer sursum-opaque">
                    <SlideView slide content=data.layout.content font_px=fit.font_px wrap=fit.wrap />
                </div>
                {masks
                    .into_iter()
                    .map(|rect| {
                        let style = format!(
                            "left:{}px;top:{}px;width:{}px;height:{}px;",
                            rect.x,
                            rect.y,
                            rect.width,
                            rect.height,
                        );
                        view! { <div class="sursum-mask sursum-mask-preview" style=style></div> }
                    })
                    .collect_view()}
            })
        })
    };

    // Test hooks: the fitted size and line count, compared with the screen in end-to-end tests.
    let font_px_attr = move || fit.with(|fit| fit.as_ref().map(|(_, fit)| fit.font_px.to_string()));
    let line_count_attr = move || fit.with(|fit| fit.as_ref().map(|(data, _)| line_count(data).to_string()));
    let opacity = move || if dimmed.get() { "0.35" } else { "1" };

    view! {
        <Stage class=format!("sursum-preview {class}")>
            <div
                class="sursum-layer"
                style:opacity=opacity
                data-font-px=font_px_attr
                data-line-count=line_count_attr
            >
                {content}
            </div>
            <div class="sursum-layer sursum-measure" node_ref=measurer></div>
        </Stage>
    }
}

/// Calls `on_shown` each time `element` goes from having no size (inside `display: none`) to
/// being laid out. Stops when the calling effect is cleaned up.
fn when_laid_out(element: web_sys::Element, on_shown: impl Fn() + 'static) {
    let has_size = |element: &web_sys::Element| element.client_width() > 0 && element.client_height() > 0;
    let mut shown = has_size(&element);
    let observed = element.clone();
    let callback = Closure::<dyn FnMut()>::new(move || {
        let now = has_size(&observed);
        if now && !shown {
            on_shown();
        }
        shown = now;
    });
    let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) else { return };
    observer.observe(&element);
    // Browser objects are not `Send`; Leptos keeps them (and the callback) in local storage
    // until the effect is cleaned up.
    let watch = StoredValue::new_local((observer, callback));
    on_cleanup(move || {
        let _ = watch.try_with_value(|(observer, _)| observer.disconnect());
    });
}

/// Number of lines on the previewed slide (exposed for tests comparing preview and screen).
fn line_count(data: &PreviewData) -> usize {
    match crate::shown::shown_slide(&data.live).map(|slide| slide.body) {
        Some(
            sursum_protocol::SlideBody::Lyrics { lines } | sursum_protocol::SlideBody::Text { lines, .. },
        ) => lines.len(),
        _ => 0,
    }
}
