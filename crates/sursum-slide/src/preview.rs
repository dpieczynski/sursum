// SPDX-License-Identifier: GPL-3.0-or-later
//! A static preview of one slide for the remote: the same stage, fitter and slide component as
//! the screen, without transitions.

use leptos::html::Div;
use leptos::prelude::*;
use sursum_protocol::{LiveSlide, ScreenLayout, Theme};
use wasm_bindgen::JsCast;

use crate::fit::{Fit, fit_deck};
use crate::slide_view::{SlideView, background_style};
use crate::stage::Stage;

/// What a preview shows.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewData {
    pub live: LiveSlide,
    pub theme: Theme,
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

    Effect::new(move |_| {
        let Some(current) = data.get() else {
            fit.set(None);
            return;
        };
        let Some(element) = measurer.get() else { return };
        let element: web_sys::HtmlElement = element.unchecked_into();
        leptos::task::spawn_local(async move {
            let result = fit_deck(&current.live.deck, &current.theme, current.layout.content, &element).await;
            // Ignore results for data that changed while measuring.
            if data.get_untracked().as_ref() == Some(&current) {
                fit.set(Some((current, result)));
            }
        });
    });

    let content = move || {
        fit.get().and_then(|(data, fit)| {
            let slide = data.live.deck.slides.get(data.live.slide_index as usize)?.clone();
            let background = background_style(&data.live.deck.background);
            let masks = data.layout.masks.clone();
            Some(view! {
                <div class="sursum-layer" style=background>
                    <SlideView slide theme=data.theme content=data.layout.content font_px=fit.font_px wrap=fit.wrap />
                </div>
                {masks
                    .into_iter()
                    .map(|rect| {
                        let style = format!("left:{}px;top:{}px;width:{}px;height:{}px;", rect.x, rect.y, rect.width, rect.height);
                        view! { <div class="sursum-mask sursum-mask-preview" style=style></div> }
                    })
                    .collect_view()}
            })
        })
    };

    view! {
        <Stage class=format!("sursum-preview {class}")>
            <div
                class="sursum-layer"
                style:opacity=move || if dimmed.get() { "0.35" } else { "1" }
                data-font-px=move || fit.with(|fit| fit.as_ref().map(|(_, fit)| fit.font_px.to_string()).unwrap_or_default())
                data-line-count=move || fit.with(|fit| fit.as_ref().map(|(data, _)| line_count(data).to_string()).unwrap_or_default())
            >
                {content}
            </div>
            <div class="sursum-layer sursum-measure" node_ref=measurer></div>
        </Stage>
    }
}

/// Number of lines on the previewed slide (exposed for tests comparing preview and screen).
fn line_count(data: &PreviewData) -> usize {
    match data.live.deck.slides.get(data.live.slide_index as usize).map(|slide| &slide.body) {
        Some(
            sursum_protocol::SlideBody::Lyrics { lines } | sursum_protocol::SlideBody::Text { lines, .. },
        ) => lines.len(),
        _ => 0,
    }
}
