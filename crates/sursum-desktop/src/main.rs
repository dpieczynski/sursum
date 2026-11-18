// SPDX-License-Identifier: GPL-3.0-or-later
//! sursum as a desktop program (spike): the same server as on the device, listening on loopback
//! only, and the remote in a window of its own instead of a browser tab. Closing the window
//! stops the server.

// No console window next to the program on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod data_dir;
mod window;

use std::net::SocketAddr;

use anyhow::Context as _;
use sursum_server::{Config, SystemPlatform};

/// Title of the window and the name people see in the taskbar.
const TITLE: &str = "Sursum";

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let data_dir = data_dir::default().context("finding a place for the data")?;
    std::fs::create_dir_all(&data_dir).with_context(|| format!("creating {}", data_dir.display()))?;
    let mut config = Config::with_data_dir(&data_dir).load_file()?;
    // Loopback only, on a free port: nothing outside this computer can reach the server, so it
    // needs no PIN (the remote is admin as long as none is set).
    config.listen = SocketAddr::from(([127, 0, 0, 1], 0));
    config.open_without_pin = true;

    // The window's toolkit wants the main thread; the server runs on the runtime's workers.
    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    let mut server = None;
    let shown = window::run(TITLE, || {
        let started = runtime.block_on(sursum_server::start(config, &SystemPlatform))?;
        let url = format!("http://{}/remote/", started.addr);
        tracing::info!(%url, data_dir = %data_dir.display(), "opening the window");
        server = Some(started);
        Ok(url)
    });
    if let Some(server) = server {
        runtime.block_on(server.stop())?;
    }
    shown
}
