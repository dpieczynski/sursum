// SPDX-License-Identifier: GPL-3.0-or-later
//! The sursum server: one process that owns all state and serves the screen and the remotes.
//!
//! - [`actor`]: the single owner of the state; applies commands in one global order.
//! - [`ws`]: WebSocket endpoints for the screen and the remotes.
//! - [`http`]: router, embedded web apps, network guard.
//! - [`db`]: SQLite persistence of the runtime state.

pub mod actor;
pub mod assets;
pub mod auth;
pub mod bundle;
pub mod config;
pub mod db;
pub mod demo;
pub mod device_backup;
pub mod display_power;
pub mod editor;
pub mod http;
pub mod import;
pub mod library;
pub mod licenses;
pub mod limits;
pub mod platform;
pub mod screen_guard;
pub mod service;
pub mod sets;
pub mod system_update;
pub mod update;
pub mod ws;

use std::net::SocketAddr;

use anyhow::Context as _;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub use config::Config;
pub use platform::{Platform, SystemPlatform};
pub use sursum_protocol::Installation;

/// Build identifier compiled into the server and the web apps; a mismatch makes clients reload.
pub const BUILD_ID: &str = match option_env!("SURSUM_BUILD_ID") {
    Some(id) => id,
    None => env!("CARGO_PKG_VERSION"),
};

/// A running server, returned by [`start`].
pub struct RunningServer {
    /// The address the server listens on (useful when the configured port was 0).
    pub addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    restart: std::sync::Arc<tokio::sync::Notify>,
}

impl RunningServer {
    /// Resolves once an update has been installed from the remote: the caller should
    /// [`stop`](Self::stop) and exit, and the service manager starts the new release.
    pub async fn restart_requested(&self) {
        self.restart.notified().await;
    }

    /// Stops accepting connections and waits for the server task to finish.
    pub async fn stop(mut self) -> anyhow::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await.context("server task panicked")?
    }
}

/// Starts the server in the background and returns once it listens.
pub async fn start(config: Config, platform: &dyn Platform) -> anyhow::Result<RunningServer> {
    // The local owner needs no PIN only because nothing outside this computer can connect.
    anyhow::ensure!(
        config.access != auth::AccessMode::LocalOwner || config.listen.ip().is_loopback(),
        "the local owner mode needs a loopback address, not {}",
        config.listen
    );
    match device_backup::apply_staged(&config.data_dir) {
        Ok(true) => tracing::info!("restored a device backup"),
        Ok(false) => {}
        Err(error) => {
            tracing::error!(?error, "could not restore the staged device backup; keeping the current data");
        }
    }
    let database = db::Database::open(&config.data_dir).context("opening the database")?;
    if let Err(error) = database.backup(&config.data_dir, "startup") {
        tracing::error!(?error, "could not write the startup backup");
    }
    let pin: Option<auth::PinHash> = database.setting(auth::PIN_SETTING).context("loading the PIN")?;
    match (&pin, config.access) {
        (_, auth::AccessMode::LocalOwner) => tracing::info!("desktop program: no PIN, loopback only"),
        (Some(_), _) => {}
        (None, auth::AccessMode::Pin) => {
            tracing::warn!("no PIN is set: the first remote to open the page must set one");
        }
        (None, auth::AccessMode::OpenForDevelopment) => tracing::warn!(
            "no PIN is set and --open-without-pin is on: any device on the local network may control everything"
        ),
    }
    let access = std::sync::Arc::new(auth::Access::new(pin, database.device_token_hashes()?, config.access));
    let state = actor::spawn(&config, database, platform)?;
    let power = display_power::spawn(&config.display_power, state.clone());
    let state_for_watchdog = state.clone();
    let presence = screen_guard::ScreenPresence::default();
    screen_guard::spawn(&presence, config.screen_guard.clone(), state.clone());
    let updates = update::Updates {
        key: config.bundle_key,
        supervised: platform.supervised(),
        restart: std::sync::Arc::default(),
        installing: std::sync::Arc::default(),
        system: system_update::SystemUpdates::detect(
            config.bundle_key,
            platform.supervised(),
            &config.system_release,
            &config.system_request,
        ),
    };
    let restart = updates.restart.clone();
    spawn_trial_commit(state.clone(), config.data_dir.clone());
    let router = http::router(state, &config, access, power, presence, updates);
    let listener =
        TcpListener::bind(config.listen).await.with_context(|| format!("binding {}", config.listen))?;
    let addr = listener.local_addr()?;
    let (shutdown, shutdown_signal) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async move {
                let _ = shutdown_signal.await;
            })
            .await
            .context("serving HTTP")
    });
    tracing::info!(%addr, build = BUILD_ID, "sursum server listening");
    service::ready();
    let watched = state_for_watchdog;
    service::spawn_watchdog(move || {
        let state = watched.clone();
        async move { state.alive().await }
    });
    Ok(RunningServer { addr, shutdown: Some(shutdown), task, restart })
}

/// After [`update::HEALTHY_AFTER`] of healthy running, ends the trial of this release if it is
/// on trial (ADR 0013, second amendment).
fn spawn_trial_commit(state: actor::StateHandle, data_dir: std::path::PathBuf) {
    let Some(own_release) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Into::into)) else {
        return;
    };
    tokio::spawn(async move {
        tokio::time::sleep(update::HEALTHY_AFTER).await;
        if !state.alive().await {
            return;
        }
        let own_release: std::path::PathBuf = own_release;
        match tokio::task::spawn_blocking(move || update::commit(&data_dir, &own_release)).await {
            Ok(Ok(Some(version))) => tracing::info!(%version, "the update passed its trial"),
            Ok(Ok(None)) => {}
            Ok(Err(error)) => tracing::error!(?error, "could not end the update's trial"),
            Err(error) => tracing::error!(?error, "the trial commit panicked"),
        }
    });
}
