// SPDX-License-Identifier: GPL-3.0-or-later
//! sursum as a desktop program (ADR 0020): the same server as on the device, as the local owner's
//! library on loopback, and the remote in a window of its own instead of a browser tab. Closing
//! the window stops the server.

// No console window next to the program on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod data_dir;
mod instance;
mod logging;
mod messages;
mod window;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use sursum_server::auth::AccessMode;
use sursum_server::{Config, Installation, RunningServer, SystemPlatform};
use window::Content;

/// Title of the window and the name people see in the taskbar.
const TITLE: &str = "Sursum";

fn main() -> anyhow::Result<()> {
    let log = logging::LogFile::default();
    log.install();
    let data_dir = data_dir::default();
    let web_data = data_dir.as_ref().map(|dir| dir.join("webview"));

    // The window's toolkit wants the main thread; the server runs on the runtime's workers.
    let runtime = tokio::runtime::Runtime::new().context("starting the async runtime")?;
    let mut running = None;
    let shown =
        window::run(TITLE, web_data.as_deref(), || match start(&runtime, data_dir.as_deref(), &log) {
            Ok(started) => {
                let origin = format!("http://{}", started.server.addr);
                tracing::info!(%origin, "opening the window");
                running = Some(started);
                Content::Remote { origin }
            }
            Err(Failure::AlreadyRunning) => Content::Message(messages::already_running()),
            Err(Failure::Broken(error)) => {
                tracing::error!(?error, "the program could not start");
                Content::Message(messages::could_not_start(log.path().as_deref()))
            }
        });
    if let Some(started) = running {
        runtime.block_on(started.server.stop())?;
    }
    shown
}

/// The server, and the lock that makes this copy of the program its only user.
struct Started {
    server: RunningServer,
    _instance: instance::Instance,
}

/// Why the remote cannot be shown.
enum Failure {
    /// Another copy of the program owns the data.
    AlreadyRunning,
    Broken(anyhow::Error),
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        Failure::Broken(error)
    }
}

/// Takes the data folder (one copy of the program at a time), starts writing the log file there
/// and starts the server on loopback as the local owner's library.
fn start(
    runtime: &tokio::runtime::Runtime,
    data_dir: Option<&Path>,
    log: &logging::LogFile,
) -> Result<Started, Failure> {
    let data_dir: PathBuf = data_dir.context("this account has no folder for application data")?.to_owned();
    std::fs::create_dir_all(&data_dir).with_context(|| format!("creating {}", data_dir.display()))?;
    let instance = instance::Instance::claim(&data_dir).context("locking the data folder")?;
    let Some(instance) = instance else {
        tracing::info!(data_dir = %data_dir.display(), "another copy holds the data folder");
        return Err(Failure::AlreadyRunning);
    };
    if let Err(error) = log.open(&data_dir) {
        tracing::warn!(?error, "no log file; logging to standard error only");
    }
    tracing::info!(data_dir = %data_dir.display(), build = sursum_server::BUILD_ID, "starting");

    let mut config = Config::with_data_dir(&data_dir).load_file()?;
    // Loopback only, on a free port: nothing outside this computer can reach the server, so its
    // user is the owner and needs no PIN (ADR 0020). There is no screen at home.
    config.listen = SocketAddr::from(([127, 0, 0, 1], 0));
    config.access = AccessMode::Local;
    config.installation = Installation::Library;
    let server = runtime.block_on(sursum_server::start(config, &SystemPlatform))?;
    Ok(Started { server, _instance: instance })
}
