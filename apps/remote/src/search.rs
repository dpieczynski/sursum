// SPDX-License-Identifier: GPL-3.0-or-later
//! Song search: the query goes to the server (`GET /api/songs`), which does all matching.

use sursum_protocol::SongSummary;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Asks the server for songs matching `query`; an empty list on any error.
pub async fn fetch_songs(query: &str) -> Vec<SongSummary> {
    let url = format!("/api/songs?q={}", js_sys::encode_uri_component(query));
    let Some(window) = web_sys::window() else { return Vec::new() };
    let Ok(response) = JsFuture::from(window.fetch_with_str(&url)).await else { return Vec::new() };
    let Ok(response) = response.dyn_into::<web_sys::Response>() else { return Vec::new() };
    let Ok(text) = response.text() else { return Vec::new() };
    let text = JsFuture::from(text).await.ok().and_then(|text| text.as_string()).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}
