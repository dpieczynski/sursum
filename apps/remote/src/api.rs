// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON over HTTP to the sursum server (search and the song editor).

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// `GET url` and decode the JSON answer; `None` on any error.
pub async fn get_json<T: DeserializeOwned>(url: &str) -> Option<T> {
    let promise = web_sys::window()?.fetch_with_str(url);
    decode(JsFuture::from(promise).await.ok()?).await
}

/// `POST url` with a JSON body and decode the JSON answer; `None` on any error.
pub async fn post_json<B: Serialize, T: DeserializeOwned>(url: &str, body: &B) -> Option<T> {
    let headers = web_sys::Headers::new().ok()?;
    headers.set("Content-Type", "application/json").ok()?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&serde_json::to_string(body).ok()?));
    let promise = web_sys::window()?.fetch_with_str_and_init(url, &init);
    decode(JsFuture::from(promise).await.ok()?).await
}

/// `POST url` with a file (e.g. a ZIP upload) as the body.
pub async fn post_file<T: DeserializeOwned>(url: &str, file: &web_sys::File) -> Option<T> {
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(file);
    let promise = web_sys::window()?.fetch_with_str_and_init(url, &init);
    decode(JsFuture::from(promise).await.ok()?).await
}

async fn decode<T: DeserializeOwned>(response: JsValue) -> Option<T> {
    let response: web_sys::Response = response.dyn_into().ok()?;
    if !response.ok() {
        return None;
    }
    let text = JsFuture::from(response.text().ok()?).await.ok()?.as_string()?;
    serde_json::from_str(&text).ok()
}
