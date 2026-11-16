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
pub mod display_power;
pub mod editor;
pub mod http;
pub mod import;
pub mod library;
pub mod licenses;
pub mod limits;
pub mod platform;
pub mod service;
pub mod sets;
pub mod ws;

use std::net::SocketAddr;

use anyhow::Context as _;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub use config::Config;
pub use platform::{Platform, SystemPlatform};

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
}

impl RunningServer {
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
    let database = db::Database::open(&config.data_dir).context("opening the database")?;
    if let Err(error) = database.backup(&config.data_dir, "startup") {
        tracing::error!(?error, "could not write the startup backup");
    }
    let pin: Option<auth::PinHash> = database.setting(auth::PIN_SETTING).context("loading the PIN")?;
    if pin.is_none() {
        tracing::warn!(
            "no PIN is set: any device on the local network can control the screen (sursum-server set-pin)"
        );
    }
    let access = std::sync::Arc::new(auth::Access::new(pin, database.device_token_hashes()?));
    let state = actor::spawn(&config, database, platform)?;
    let power = display_power::spawn(&config.display_power, state.clone());
    let state_for_watchdog = state.clone();
    let router = http::router(state, &config, access, power);
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
    Ok(RunningServer { addr, shutdown: Some(shutdown), task })
}
