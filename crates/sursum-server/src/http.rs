// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP routes: the embedded web apps, the WebSocket endpoints and a health check, behind a
//! guard that only lets local-network clients in.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Path, Query, Request, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use rust_embed::Embed;
use serde::Deserialize;
use sursum_protocol::Role;

use crate::actor::StateHandle;
use crate::config::Config;
use crate::ws;

/// The screen app as built by Trunk (`apps/screen/dist`).
#[derive(Embed)]
#[folder = "../../apps/screen/dist"]
#[allow_missing = true]
struct ScreenAssets;

/// The remote app as built by Trunk (`apps/remote/dist`).
#[derive(Embed)]
#[folder = "../../apps/remote/dist"]
#[allow_missing = true]
struct RemoteAssets;

/// Shown on `/screen/` if the screen app was not built: black, never an error message.
const SCREEN_FALLBACK: &str = "<!doctype html><html style=\"background:#000\"><head><meta charset=\"utf-8\">\
<style>html,body{margin:0;background:#000;cursor:none}</style></head><body></body></html>";

#[derive(Clone)]
struct App {
    state: StateHandle,
    allowed_origins: Arc<Vec<String>>,
    allow_public_clients: bool,
    wasm_delay_ms: u64,
}

/// Builds the router.
pub fn router(state: StateHandle, config: &Config) -> Router {
    let app = App {
        state,
        allowed_origins: Arc::new(config.allowed_origins.clone()),
        allow_public_clients: config.allow_public_clients,
        wasm_delay_ms: config.wasm_delay_ms,
    };
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/remote/") }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/licenses", get(|| async { axum::response::Html(crate::licenses::PAGE.as_str()) }))
        .route("/api/songs", get(search_songs))
        .route("/api/songs/preview", axum::routing::post(preview_song))
        .route("/api/songs/save", axum::routing::post(save_song))
        .route("/api/songs/{id}/draft", get(song_draft))
        .route("/ws/screen", get(screen_socket))
        .route("/ws/remote", get(remote_socket))
        .route("/screen", get(|| async { Redirect::permanent("/screen/") }))
        .route("/screen/", get(screen_index))
        .route("/screen/{*path}", get(screen_asset))
        .route("/remote", get(|| async { Redirect::permanent("/remote/") }))
        .route("/remote/", get(remote_index))
        .route("/remote/{*path}", get(remote_asset))
        .layer(middleware::from_fn_with_state(app.clone(), local_network_only))
        .with_state(app)
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    30
}

/// `GET /api/songs?q=…`: search by songbook number, title or lyrics (ADR 0006).
async fn search_songs(State(app): State<App>, Query(query): Query<SearchQuery>) -> Response {
    axum::Json(app.state.search(query.q, query.limit.min(100)).await).into_response()
}

/// `GET /api/songs/{id}/draft`: a song in the editor's text form.
async fn song_draft(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.state.draft(sursum_protocol::SongId(id)).await {
        Some(draft) => axum::Json(draft).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /api/songs/preview`: the slides a draft would produce (nothing is saved).
async fn preview_song(axum::Json(draft): axum::Json<sursum_protocol::SongDraft>) -> Response {
    axum::Json(crate::editor::preview(&draft)).into_response()
}

/// `POST /api/songs/save`: saves a new or edited song.
async fn save_song(
    State(app): State<App>,
    axum::Json(draft): axum::Json<sursum_protocol::SongDraft>,
) -> Response {
    axum::Json(app.state.save_song(draft).await).into_response()
}

async fn screen_socket(State(app): State<App>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    socket(app, &headers, upgrade, Role::Screen)
}

async fn remote_socket(State(app): State<App>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    socket(app, &headers, upgrade, Role::Remote)
}

fn socket(app: App, headers: &HeaderMap, upgrade: WebSocketUpgrade, role: Role) -> Response {
    if !ws::origin_allowed(headers, &app.allowed_origins) {
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade.on_upgrade(move |socket| ws::session(socket, app.state, role))
}

async fn screen_index(State(app): State<App>) -> Response {
    serve::<ScreenAssets>(&app, "index.html").await.unwrap_or_else(|| html_response(SCREEN_FALLBACK))
}

async fn screen_asset(State(app): State<App>, Path(path): Path<String>) -> Response {
    serve::<ScreenAssets>(&app, &path).await.unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

async fn remote_index(State(app): State<App>) -> Response {
    serve::<RemoteAssets>(&app, "index.html").await.unwrap_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, "The remote app is not built. Run `just build`.").into_response()
    })
}

async fn remote_asset(State(app): State<App>, Path(path): Path<String>) -> Response {
    serve::<RemoteAssets>(&app, &path).await.unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

/// Serves an embedded file. `index.html` is never cached (it names the hashed assets); the
/// hashed assets Trunk produces are cached for a long time.
async fn serve<A: Embed>(app: &App, path: &str) -> Option<Response> {
    let file = A::get(path)?;
    if cfg!(feature = "e2e-hooks") && app.wasm_delay_ms > 0 && path.ends_with(".wasm") {
        tokio::time::sleep(std::time::Duration::from_millis(app.wasm_delay_ms)).await;
    }
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path == "index.html" { "no-cache" } else { "public, max-age=31536000, immutable" };
    let mut response = file.data.into_owned().into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).ok()?);
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    Some(response)
}

fn html_response(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// Rejects clients outside loopback and private networks (unless configured otherwise).
async fn local_network_only(
    State(app): State<App>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if app.allow_public_clients || is_local(peer.ip()) {
        next.run(request).await
    } else {
        tracing::warn!(%peer, "refused a client from outside the local network");
        StatusCode::FORBIDDEN.into_response()
    }
}

/// True for loopback, private (RFC 1918 / ULA) and link-local addresses.
fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local(IpAddr::V4(v4)),
            None => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_addresses_are_recognized() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.5",
            "172.16.0.1",
            "169.254.1.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.2",
        ] {
            assert!(is_local(ip.parse().unwrap()), "{ip} should be local");
        }
        for ip in ["8.8.8.8", "2001:4860:4860::8888", "::ffff:8.8.8.8"] {
            assert!(!is_local(ip.parse().unwrap()), "{ip} should not be local");
        }
    }
}
