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
use sursum_protocol::{DeviceView, PairOutcome, PairRequest, Role, SessionInfo};

use crate::actor::StateHandle;
use crate::auth::{self, Access, PinCheck};
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
    data_dir: Arc<std::path::PathBuf>,
    access: Arc<Access>,
    allowed_origins: Arc<Vec<String>>,
    allowed_hosts: Arc<Vec<String>>,
    power: crate::display_power::PowerHandle,
    presence: crate::screen_guard::ScreenPresence,
    allow_public_clients: bool,
    wasm_delay_ms: u64,
    updates: crate::update::Updates,
    installation: sursum_protocol::Installation,
}

/// Builds the router.
pub fn router(
    state: StateHandle,
    config: &Config,
    access: Arc<Access>,
    power: crate::display_power::PowerHandle,
    presence: crate::screen_guard::ScreenPresence,
    updates: crate::update::Updates,
) -> Router {
    let app = App {
        power,
        presence,
        state,
        data_dir: Arc::new(config.data_dir.clone()),
        access,
        allowed_origins: Arc::new(config.allowed_origins.clone()),
        allowed_hosts: Arc::new(config.allowed_hosts.clone()),
        allow_public_clients: config.allow_public_clients,
        wasm_delay_ms: config.wasm_delay_ms,
        updates,
        installation: config.installation,
    };
    // Everything that controls the screen or changes data needs a paired device (once a PIN is
    // set); a paired device may do all of it (ADR 0024).
    let protected = Router::new()
        .route("/ws/remote", get(remote_socket))
        .route("/api/songs", get(search_songs))
        .route("/api/songs/preview", axum::routing::post(preview_song))
        .route("/api/songs/save", axum::routing::post(save_song))
        .route("/api/songs/{id}/draft", get(song_draft))
        .route("/api/songs/review", get(songs_in_review))
        .route("/api/songs/{id}/versions", get(song_versions))
        .route("/api/songs/{id}/keep", axum::routing::post(keep_version))
        .route("/api/library/export", get(export_library))
        .route("/api/backup", get(download_backup))
        .route("/api/update/status", get(update_status))
        .route(
            "/api/assets",
            axum::routing::post(upload_asset)
                .layer(axum::extract::DefaultBodyLimit::max(crate::assets::MAX_ASSET_BYTES + 1))
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route(
            "/api/import/opensong",
            axum::routing::post(import_opensong)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_IMPORT_BYTES))
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route(
            "/api/library/merge",
            axum::routing::post(merge_library)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_IMPORT_BYTES))
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route(
            "/api/update",
            axum::routing::post(install_update)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_UPDATE_BYTES))
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route(
            "/api/backup/restore",
            axum::routing::post(restore_backup)
                .layer(axum::extract::DefaultBodyLimit::disable())
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route(
            "/api/system/update",
            axum::routing::post(install_system_update)
                .layer(axum::extract::DefaultBodyLimit::disable())
                .layer(middleware::from_fn(require_upload_header)),
        )
        .route("/api/assets/cleanup", axum::routing::post(clean_up_pictures))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/{id}/revoke", axum::routing::post(revoke_device))
        .route_layer(middleware::from_fn_with_state(app.clone(), require_pairing));
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/remote/") }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/licenses", get(|| async { axum::response::Html(crate::licenses::PAGE.as_str()) }))
        .route("/api/session", get(session))
        .route("/api/pair", axum::routing::post(pair))
        .route("/api/setup", axum::routing::post(setup))
        .route("/ws/screen", get(screen_socket))
        .route("/assets/{id}", get(asset))
        .route("/screen", get(|| async { Redirect::permanent("/screen/") }))
        .route("/screen/", get(screen_index))
        .route("/screen/{*path}", get(screen_asset))
        .route("/remote", get(|| async { Redirect::permanent("/remote/") }))
        .route("/remote/", get(remote_index))
        .route("/remote/{*path}", get(remote_asset))
        .merge(protected)
        .layer(middleware::from_fn_with_state(app.clone(), known_host))
        .layer(middleware::from_fn_with_state(app.clone(), local_network_only))
        .with_state(app)
}

/// Refuses requests from unpaired browsers once a PIN is set.
async fn require_pairing(State(app): State<App>, request: Request, next: Next) -> Response {
    if app.access.allows(request.headers()) {
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// `GET /api/session`: whether a PIN is set and whether this browser is paired.
async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    let info = SessionInfo {
        pin_required: app.access.pin_required(),
        paired: app.access.allows(&headers),
        setup_needed: app.access.setup_needed(),
        installation: app.installation,
    };
    axum::Json(info).into_response()
}

/// `GET /api/devices`: paired remotes.
async fn list_devices(State(app): State<App>, headers: HeaderMap) -> Response {
    let this_device = app.access.device(&headers);
    let devices: Vec<DeviceView> = app
        .state
        .devices()
        .await
        .into_iter()
        .map(|(device_id, name, token_hash, paired_at)| DeviceView {
            device_id,
            name,
            paired_at,
            this_device: this_device.as_deref() == Some(token_hash.as_str()),
        })
        .collect();
    axum::Json(devices).into_response()
}

/// `POST /api/devices/{id}/revoke`: forgets a remote; it must pair again with the PIN.
async fn revoke_device(State(app): State<App>, Path(device_id): Path<String>) -> Response {
    match app.state.revoke_device(device_id).await {
        Some(hash) => {
            app.access.revoke(&hash);
            StatusCode::NO_CONTENT.into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /api/pair`: pairs this browser with the PIN (rate-limited per address).
async fn pair(
    State(app): State<App>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Json(request): axum::Json<PairRequest>,
) -> Response {
    match app.access.check_pin(peer.ip(), &request.pin, std::time::Instant::now()) {
        PinCheck::Correct => {}
        PinCheck::Wrong => {
            return (StatusCode::UNAUTHORIZED, axum::Json(PairOutcome::WrongPin)).into_response();
        }
        PinCheck::Locked(wait) => {
            let retry_after_s = u32::try_from(wait.as_secs().max(1)).unwrap_or(u32::MAX);
            return (StatusCode::TOO_MANY_REQUESTS, axum::Json(PairOutcome::Locked { retry_after_s }))
                .into_response();
        }
    }
    match pair_this_browser(&app, &request.name).await {
        Ok(cookie) => ([(header::SET_COOKIE, cookie)], axum::Json(PairOutcome::Paired)).into_response(),
        Err(status) => status.into_response(),
    }
}

/// `POST /api/setup`: sets the first PIN (only while none is set), names the device's church
/// (ADR 0025) and pairs this browser, so whoever installs the device can go on configuring it.
async fn setup(State(app): State<App>, axum::Json(request): axum::Json<PairRequest>) -> Response {
    let pin = request.pin.trim();
    if !auth::valid_pin(pin) {
        return (StatusCode::BAD_REQUEST, axum::Json(PairOutcome::InvalidPin)).into_response();
    }
    if sursum_core::churches::church_id(&request.church).is_none() {
        return (StatusCode::BAD_REQUEST, axum::Json(PairOutcome::ChurchMissing)).into_response();
    }
    let Ok(hash) = auth::PinHash::new(pin) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if !app.access.setup_needed() || !app.access.claim_pin(hash.clone()) {
        return (StatusCode::CONFLICT, axum::Json(PairOutcome::AlreadySet)).into_response();
    }
    if !app.state.set_pin(hash).await {
        app.access.release_pin();
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let name_church = sursum_protocol::Command::NameChurch { name: request.church.clone() };
    let named = app
        .state
        .command(sursum_protocol::ClientId::from("setup"), sursum_protocol::Rev(u32::MAX), name_church)
        .await;
    if !matches!(named, sursum_protocol::CmdOutcome::Applied { .. }) {
        tracing::error!(?named, "could not name the church during setup");
    }
    match pair_this_browser(&app, &request.name).await {
        Ok(cookie) => ([(header::SET_COOKIE, cookie)], axum::Json(PairOutcome::Paired)).into_response(),
        Err(status) => status.into_response(),
    }
}

/// Stores a new device and returns the cookie that pairs the browser.
async fn pair_this_browser(app: &App, name: &str) -> Result<HeaderValue, StatusCode> {
    let (token, hash) = auth::new_token().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let name = if name.trim().is_empty() { "Pilot".to_owned() } else { name.trim().to_owned() };
    if !app.state.add_device(name, hash.clone()).await {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    app.access.add_token(hash);
    Ok(HeaderValue::from_str(&auth::cookie_header(&token)).expect("token is hex"))
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

/// Largest accepted ZIP upload (a large OpenSong library with images fits easily).
const MAX_IMPORT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize)]
struct ImportQuery {
    #[serde(default = "default_songbook")]
    songbook: String,
}

fn default_songbook() -> String {
    "OpenSong".to_owned()
}

/// `POST /api/import/opensong`: the body is a ZIP of an OpenSong folder (or of `Songs/`).
async fn import_opensong(
    State(app): State<App>,
    Query(query): Query<ImportQuery>,
    body: axum::body::Bytes,
) -> Response {
    let parsed = tokio::task::spawn_blocking(move || crate::import::read_zip(&body, &query.songbook)).await;
    match parsed {
        Ok(Ok(library)) => axum::Json(app.state.import(library).await).into_response(),
        Ok(Err(error)) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// An application bundle is one compressed binary of a few tens of megabytes.
const MAX_UPDATE_BYTES: usize = 128 * 1024 * 1024;

/// `GET /api/update/status`: the running version and how the last update went.
async fn update_status(State(app): State<App>) -> Response {
    let data_dir = app.data_dir.clone();
    let can_install = app.updates.can_install();
    let system = app.updates.system.clone();
    match tokio::task::spawn_blocking(move || {
        let system = system.status(&data_dir);
        crate::update::status(&data_dir, crate::BUILD_ID, can_install, system)
    })
    .await
    {
        Ok(status) => axum::Json(status).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/update`: installs a signed bundle, answers, then restarts into it.
async fn install_update(State(app): State<App>, body: axum::body::Bytes) -> Response {
    use sursum_protocol::{InstallOutcome, InstallRefusal};
    let refuse = |reason| axum::Json(InstallOutcome::Refused { reason }).into_response();
    let Some(key) = app.updates.key else { return refuse(InstallRefusal::NoKey) };
    if !app.updates.supervised {
        return refuse(InstallRefusal::NotSupervised);
    }
    let _installing = app.updates.installing.lock().await;
    let data_dir = app.data_dir.clone();
    match tokio::task::spawn_blocking(move || crate::update::install(&data_dir, &body, &key)).await {
        Ok(Ok(version)) => {
            tracing::info!(%version, "update installed; restarting into it");
            let restart = app.updates.restart.clone();
            tokio::spawn(async move {
                // Let the answer reach the remote first.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                restart.notify_one();
            });
            axum::Json(InstallOutcome::Installed { version }).into_response()
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "update refused");
            refuse(error.refusal())
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/assets`: stores an uploaded picture and returns its id (JSON string).
async fn upload_asset(State(app): State<App>, body: axum::body::Bytes) -> Response {
    let data_dir = app.data_dir.clone();
    match tokio::task::spawn_blocking(move || crate::assets::store(&data_dir, &body)).await {
        Ok(Ok(id)) => axum::Json(id).into_response(),
        Ok(Err(error @ crate::assets::AssetError::UnsupportedType)) => {
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, error.to_string()).into_response()
        }
        Ok(Err(error @ crate::assets::AssetError::TooLarge)) => {
            (StatusCode::PAYLOAD_TOO_LARGE, error.to_string()).into_response()
        }
        Ok(Err(error @ crate::assets::AssetError::Full)) => {
            (StatusCode::INSUFFICIENT_STORAGE, error.to_string()).into_response()
        }
        Ok(Err(error)) => {
            tracing::error!(%error, "could not store an uploaded picture");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/system/update` (ADR 0019): receives a `.sursum-system` into the data
/// directory (streamed: it is hundreds of megabytes), verifies it and asks the root unit to write
/// it to the other slot; the device then restarts into it on trial.
async fn install_system_update(State(app): State<App>, body: axum::body::Body) -> Response {
    use futures_util::StreamExt as _;
    use sursum_protocol::{InstallOutcome, InstallRefusal};
    let refuse = |reason| axum::Json(InstallOutcome::Refused { reason }).into_response();
    let system = app.updates.system.clone();
    let data_dir = app.data_dir.clone();
    match system.status(&data_dir) {
        None => return refuse(InstallRefusal::NotSupervised),
        Some(_) if system.key.is_none() => return refuse(InstallRefusal::NoKey),
        Some(status) if !status.can_install && system.supervised => return refuse(InstallRefusal::Busy),
        Some(status) if !status.can_install => return refuse(InstallRefusal::NotSupervised),
        Some(_) => {}
    }
    let _installing = app.updates.installing.lock().await;
    let incoming = match system.incoming(&data_dir) {
        Ok(incoming) => incoming,
        Err(error) => {
            tracing::error!(?error, "could not receive a system update");
            return refuse(InstallRefusal::Failed);
        }
    };
    let received = async {
        let mut file = std::fs::File::create(&incoming)?;
        let mut total: u64 = 0;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(std::io::Error::other)?;
            total += chunk.len() as u64;
            if total > crate::system_update::MAX_SYSTEM_BYTES {
                return Err(std::io::Error::other("larger than any system update"));
            }
            std::io::Write::write_all(&mut file, &chunk)?;
        }
        file.sync_all()?;
        Ok::<_, std::io::Error>(())
    }
    .await;
    if let Err(error) = received {
        tracing::warn!(%error, "system update upload failed");
        let _ = std::fs::remove_file(&incoming);
        return refuse(InstallRefusal::Failed);
    }
    let staged = {
        let incoming = incoming.clone();
        tokio::task::spawn_blocking(move || system.stage(&data_dir, &incoming)).await
    };
    let _ = std::fs::remove_file(&incoming);
    match staged {
        Ok(Ok(version)) => {
            tracing::info!(%version, "system update verified; the device writes it and restarts into it");
            axum::Json(InstallOutcome::Installed { version }).into_response()
        }
        Ok(Err(crate::system_update::SystemUpdateError::AlreadyCurrent)) => {
            refuse(InstallRefusal::AlreadyCurrent)
        }
        Ok(Err(error @ crate::system_update::SystemUpdateError::BadFile(_))) => {
            tracing::warn!(%error, "system update refused");
            refuse(InstallRefusal::BadBundle)
        }
        Ok(Err(error)) => {
            tracing::error!(%error, "could not stage a system update");
            refuse(InstallRefusal::Failed)
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/assets/cleanup`: removes pictures nothing uses (or counts them first).
async fn clean_up_pictures(
    State(app): State<App>,
    axum::Json(request): axum::Json<sursum_protocol::PictureCleanupRequest>,
) -> Response {
    match app.state.clean_up_pictures(request.dry_run).await {
        Some(cleanup) => axum::Json(cleanup).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /assets/{id}`: a stored picture. Open like the screen itself (the screen needs it); ids
/// are content hashes, so the answer never changes and may be cached forever.
async fn asset(State(app): State<App>, Path(id): Path<String>) -> Response {
    let Some((path, mime)) = crate::assets::locate(&app.data_dir, &id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::task::spawn_blocking(move || std::fs::read(path)).await {
        Ok(Ok(bytes)) => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
                (header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable")),
            ],
            bytes,
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /api/library/export`: the whole library as a `.sursum` file (also a backup).
async fn export_library(State(app): State<App>) -> Response {
    match app.state.export_library().await {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
                (
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_static("attachment; filename=\"biblioteka.sursum\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /api/backup`: a whole-device backup (`.sursum-backup`, ADR 0018), streamed from a
/// temporary file (pictures can make it large) that is removed once sent.
async fn download_backup(State(app): State<App>) -> Response {
    let data_dir = app.data_dir.clone();
    let created = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let mut temporary = crate::device_backup::TemporaryFile::new(&data_dir)?;
        crate::device_backup::create(&data_dir, temporary.file())?;
        std::io::Seek::rewind(temporary.file())?;
        Ok(temporary)
    })
    .await;
    let temporary = match created {
        Ok(Ok(temporary)) => temporary,
        Ok(Err(error)) => {
            tracing::error!(?error, "could not create a device backup");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let chunks = futures_util::stream::try_unfold(temporary, |mut temporary| async move {
        let read = tokio::task::spawn_blocking(move || {
            let mut chunk = vec![0; 256 * 1024];
            let read = std::io::Read::read(temporary.file(), &mut chunk)?;
            chunk.truncate(read);
            Ok::<_, std::io::Error>((chunk, temporary))
        })
        .await
        .map_err(std::io::Error::other)??;
        let (chunk, temporary) = read;
        Ok::<_, std::io::Error>((!chunk.is_empty()).then(|| (axum::body::Bytes::from(chunk), temporary)))
    });
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/zip")),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"kopia.sursum-backup\""),
            ),
        ],
        axum::body::Body::from_stream(chunks),
    )
        .into_response()
}

/// `POST /api/backup/restore`: checks and stages a backup, answers, then restarts so the
/// next start puts it in place.
async fn restore_backup(State(app): State<App>, body: axum::body::Body) -> Response {
    use futures_util::StreamExt as _;
    use sursum_protocol::{RestoreOutcome, RestoreRefusal};
    let refuse = |reason| axum::Json(RestoreOutcome::Refused { reason }).into_response();
    if !app.updates.supervised {
        return refuse(RestoreRefusal::NotSupervised);
    }
    let _busy = app.updates.installing.lock().await;
    let data_dir = app.data_dir.clone();
    let mut upload = match crate::device_backup::TemporaryFile::new(&data_dir) {
        Ok(upload) => upload,
        Err(error) => {
            tracing::error!(?error, "could not store an uploaded backup");
            return refuse(RestoreRefusal::Failed);
        }
    };
    let mut received: u64 = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { return StatusCode::BAD_REQUEST.into_response() };
        received += chunk.len() as u64;
        if received > crate::device_backup::MAX_BACKUP_BYTES {
            return refuse(RestoreRefusal::TooLarge);
        }
        if let Err(error) = std::io::Write::write_all(upload.file(), &chunk) {
            tracing::error!(?error, "could not store an uploaded backup");
            return refuse(RestoreRefusal::Failed);
        }
    }
    let staged = tokio::task::spawn_blocking(move || {
        std::io::Seek::rewind(upload.file())
            .map_err(|error| crate::device_backup::RestoreError::Failed(error.into()))?;
        crate::device_backup::stage(&data_dir, upload.file())
    })
    .await;
    match staged {
        Ok(Ok(summary)) => {
            tracing::info!(
                songs = summary.songs,
                sets = summary.sets,
                "device backup staged; restarting into it"
            );
            let restart = app.updates.restart.clone();
            tokio::spawn(async move {
                // Let the answer reach the remote first.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                restart.notify_one();
            });
            axum::Json(RestoreOutcome::Restored { summary }).into_response()
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "device backup refused");
            refuse(error.refusal())
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct MergeQuery {
    #[serde(default)]
    dry_run: bool,
}

/// `POST /api/library/merge?dry_run=true`: merges a `.sursum` file (or reports what would happen).
async fn merge_library(
    State(app): State<App>,
    Query(query): Query<MergeQuery>,
    body: axum::body::Bytes,
) -> Response {
    let songs = match tokio::task::spawn_blocking(move || crate::library::read_bundle(&body)).await {
        Ok(Ok(songs)) => songs,
        Ok(Err(error)) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match app.state.merge_library(songs, query.dry_run).await {
        Some(report) => axum::Json(report).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /api/songs/{id}/draft`: a song in the editor's text form.
async fn song_draft(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.state.draft(sursum_protocol::SongId(id)).await {
        Some(draft) => axum::Json(draft).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /api/songs/review`: songs changed in two places that wait for review (ADR 0025).
async fn songs_in_review(State(app): State<App>) -> Response {
    axum::Json(app.state.songs_in_review().await).into_response()
}

/// `GET /api/songs/{id}/versions`: every live version of a song, for the comparison screen.
async fn song_versions(State(app): State<App>, Path(id): Path<String>) -> Response {
    match app.state.song_versions(sursum_protocol::SongId(id)).await {
        Some(versions) => axum::Json(versions).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /api/songs/{id}/keep`: keeps one version and replaces the others.
async fn keep_version(
    State(app): State<App>,
    Path(id): Path<String>,
    axum::Json(keep): axum::Json<sursum_protocol::KeepVersion>,
) -> Response {
    axum::Json(app.state.keep_version(sursum_protocol::SongId(id), keep.version).await).into_response()
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

async fn screen_socket(
    State(app): State<App>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    socket(app, &headers, upgrade, Role::Screen, peer.ip().is_loopback())
}

async fn remote_socket(State(app): State<App>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    socket(app, &headers, upgrade, Role::Remote, false)
}

fn socket(
    app: App,
    headers: &HeaderMap,
    upgrade: WebSocketUpgrade,
    role: Role,
    local_peer: bool,
) -> Response {
    if !ws::origin_allowed(headers, &app.allowed_origins) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let guard = ws::Guard {
        access: app.access.clone(),
        headers: headers.clone(),
        power: app.power.clone(),
        presence: app.presence.clone(),
        local_peer,
    };
    upgrade.on_upgrade(move |socket| ws::session(socket, app.state, role, guard))
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
/// The header the remote sends with file uploads. Pages on other sites cannot add a custom header
/// without a CORS preflight, which this server never grants, so they cannot post files here even
/// while no PIN is set (raw bodies would otherwise pass as "simple" cross-site requests).
pub const UPLOAD_HEADER: &str = "x-sursum-upload";

async fn require_upload_header(request: Request, next: Next) -> Response {
    if request.headers().contains_key(UPLOAD_HEADER) {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

/// Answers only to host names this device can have: IP addresses, `localhost`, `*.local` (mDNS)
/// and configured names. A page on the internet that re-points its own name to the device (DNS
/// rebinding) arrives with its own name in `Host` and is refused.
async fn known_host(State(app): State<App>, request: Request, next: Next) -> Response {
    let host = request.headers().get(header::HOST).and_then(|value| value.to_str().ok());
    if host.is_none_or(|host| host_allowed(host, &app.allowed_hosts)) {
        next.run(request).await
    } else {
        tracing::warn!(?host, "refused a request for an unknown host name");
        StatusCode::MISDIRECTED_REQUEST.into_response()
    }
}

fn host_allowed(host: &str, allowed: &[String]) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => host.rsplit_once(':').map_or(host, |(name, port)| {
            if port.bytes().all(|byte| byte.is_ascii_digit()) { name } else { host }
        }),
    };
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name.parse::<IpAddr>().is_ok()
        || name == "localhost"
        || name.ends_with(".local")
        || allowed.iter().any(|allowed| allowed.eq_ignore_ascii_case(&name))
}

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

    #[test]
    fn only_names_of_this_device_are_answered() {
        let allowed = vec!["sursum.parafia.lan".to_owned()];
        for host in [
            "192.168.1.50",
            "192.168.1.50:8080",
            "[fd00::5]:8080",
            "localhost:8080",
            "sursum.local",
            "SURSUM.LOCAL.",
            "sursum.parafia.lan:80",
        ] {
            assert!(host_allowed(host, &allowed), "{host} should be allowed");
        }
        for host in
            ["evil.example.com", "evil.example.com:8080", "192.168.1.50.evil.com", "localhost.evil.com"]
        {
            assert!(!host_allowed(host, &allowed), "{host} should be refused");
        }
    }
}
