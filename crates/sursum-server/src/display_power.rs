// SPDX-License-Identifier: GPL-3.0-or-later
//! Switching the projector or monitor on and off (ADR 0012).
//!
//! One background task owns the display connection: it runs power commands from remotes and asks
//! for the power state every [`POLL`] (more often while a projector warms up or cools down). The
//! state goes to the state actor like any other change, so remotes see it in their snapshot; the
//! actor itself never waits for a slow projector. HDMI-CEC follows on the Pi (M4, hardware spike).

use std::time::Duration;

use sursum_core::display_power::{
    Greeting, PJLINK_PORT, POWER_QUERY, PjLinkError, PowerReply, auth_prefix, parse_greeting,
    parse_power_reply, power_command,
};
use sursum_protocol::{DisplayPowerState, ErrorCode, PowerAction};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use crate::actor::StateHandle;
use crate::config::DisplayPowerConfig;

/// Normal interval between state queries.
const POLL: Duration = Duration::from_secs(30);
/// Interval while the state is changing (warming up, cooling down).
const POLL_CHANGING: Duration = Duration::from_secs(5);
/// Longest wait for a projector to connect and answer one command.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a power command or query failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PowerError {
    #[error("no power control is configured")]
    NotConfigured,
    #[error("the display does not answer: {0}")]
    Unreachable(String),
    #[error("the display rejected the password")]
    AuthFailed,
    #[error("the display is busy (e.g. cooling down)")]
    Busy,
    #[error("the display reports a problem: {0}")]
    Failure(String),
}

impl PowerError {
    /// The code a remote gets back.
    pub fn code(&self) -> ErrorCode {
        match self {
            PowerError::NotConfigured | PowerError::Unreachable(_) | PowerError::Failure(_) => {
                ErrorCode::DisplayUnreachable
            }
            PowerError::AuthFailed => ErrorCode::DisplayAuthFailed,
            PowerError::Busy => ErrorCode::DisplayBusy,
        }
    }

    /// The `reason` shown as "unavailable" in the remote's status.
    fn reason(&self) -> &'static str {
        match self {
            PowerError::NotConfigured => "not_configured",
            PowerError::Unreachable(_) => "unreachable",
            PowerError::AuthFailed => "auth_failed",
            PowerError::Busy => "busy",
            PowerError::Failure(_) => "failure",
        }
    }
}

impl From<PjLinkError> for PowerError {
    fn from(error: PjLinkError) -> Self {
        match error {
            PjLinkError::AuthFailed => PowerError::AuthFailed,
            PjLinkError::Busy => PowerError::Busy,
            other => PowerError::Failure(other.to_string()),
        }
    }
}

/// The configured display.
#[derive(Debug, Clone)]
enum Display {
    None,
    PjLink { address: String, password: Option<String> },
}

impl Display {
    fn from_config(config: &DisplayPowerConfig) -> Self {
        match config {
            DisplayPowerConfig::None => Display::None,
            DisplayPowerConfig::Pjlink { host, password } => {
                let address = if host.contains(':') { host.clone() } else { format!("{host}:{PJLINK_PORT}") };
                Display::PjLink { address, password: password.clone() }
            }
        }
    }

    async fn set(&self, action: PowerAction) -> Result<(), PowerError> {
        match self {
            Display::None => Err(PowerError::NotConfigured),
            Display::PjLink { address, password } => {
                match pjlink(address, password.as_deref(), power_command(action)).await? {
                    PowerReply::Accepted => Ok(()),
                    PowerReply::State(_) => Err(PowerError::Failure("a state instead of OK".into())),
                }
            }
        }
    }

    async fn state(&self) -> Result<DisplayPowerState, PowerError> {
        match self {
            Display::None => Err(PowerError::NotConfigured),
            Display::PjLink { address, password } => {
                match pjlink(address, password.as_deref(), POWER_QUERY).await? {
                    PowerReply::State(state) => Ok(state),
                    PowerReply::Accepted => Err(PowerError::Failure("OK instead of a state".into())),
                }
            }
        }
    }
}

/// One PJLink exchange on a fresh connection: greeting, (password), one command, one answer.
async fn pjlink(address: &str, password: Option<&str>, command: &str) -> Result<PowerReply, PowerError> {
    let exchange = async {
        let stream =
            TcpStream::connect(address).await.map_err(|error| PowerError::Unreachable(error.to_string()))?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let greeting = read_line(&mut reader).await?;
        let prefix = match parse_greeting(&greeting)? {
            Greeting::Open => String::new(),
            Greeting::Challenge { seed } => auth_prefix(&seed, password.ok_or(PowerError::AuthFailed)?),
        };
        let line = format!("{prefix}{command}");
        writer
            .write_all(line.as_bytes())
            .await
            .map_err(|error| PowerError::Unreachable(error.to_string()))?;
        let reply = read_line(&mut reader).await?;
        Ok(parse_power_reply(&reply)?)
    };
    tokio::time::timeout(EXCHANGE_TIMEOUT, exchange)
        .await
        .unwrap_or_else(|_| Err(PowerError::Unreachable("timed out".into())))
}

/// Reads up to (and without) the CR that ends every PJLink line.
async fn read_line(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<String, PowerError> {
    let mut bytes = Vec::new();
    let read = reader
        .read_until(b'\r', &mut bytes)
        .await
        .map_err(|error| PowerError::Unreachable(error.to_string()))?;
    if read == 0 {
        return Err(PowerError::Unreachable("connection closed".into()));
    }
    Ok(String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_owned())
}

/// Passes state changes (only changes) on to the state actor.
struct Reporter {
    state: StateHandle,
    last: Option<DisplayPowerState>,
}

impl Reporter {
    async fn report(&mut self, result: Result<DisplayPowerState, PowerError>) {
        let current = result
            .unwrap_or_else(|error| DisplayPowerState::Unavailable { reason: error.reason().to_owned() });
        if self.last.as_ref() != Some(&current) {
            self.state.display_power_changed(current.clone()).await;
            self.last = Some(current);
        }
    }
}

type Job = (PowerAction, oneshot::Sender<Result<(), PowerError>>);

/// Sends power commands to the background task.
#[derive(Clone)]
pub struct PowerHandle {
    jobs: mpsc::Sender<Job>,
}

impl PowerHandle {
    /// Runs a power command and waits for the display's answer.
    pub async fn run(&self, action: PowerAction) -> Result<(), PowerError> {
        let (reply, result) = oneshot::channel();
        self.jobs.send((action, reply)).await.map_err(|_| PowerError::NotConfigured)?;
        result.await.unwrap_or(Err(PowerError::NotConfigured))
    }
}

/// Starts the background task for the configured display.
pub fn spawn(config: &DisplayPowerConfig, state: StateHandle) -> PowerHandle {
    let display = Display::from_config(config);
    let (jobs, mut inbox) = mpsc::channel::<Job>(8);
    tokio::spawn(async move {
        let mut reporter = Reporter { state, last: None };
        reporter.report(display.state().await).await;
        if matches!(display, Display::None) {
            // Nothing to poll; keep answering commands with "not configured".
            while let Some((_, reply)) = inbox.recv().await {
                let _ = reply.send(Err(PowerError::NotConfigured));
            }
            return;
        }
        loop {
            let changing =
                matches!(reporter.last, Some(DisplayPowerState::WarmingUp | DisplayPowerState::CoolingDown));
            let wait = if changing { POLL_CHANGING } else { POLL };
            tokio::select! {
                job = inbox.recv() => {
                    let Some((action, reply)) = job else { return };
                    let result = display.set(action).await;
                    if let Err(error) = &result {
                        tracing::warn!(%error, ?action, "display power command failed");
                    } else {
                        tracing::info!(?action, "display power command sent");
                    }
                    let _ = reply.send(result);
                }
                () = tokio::time::sleep(wait) => {}
            }
            reporter.report(display.state().await).await;
        }
    });
    PowerHandle { jobs }
}
