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

/// A projector or monitor whose power the server can switch (ADR 0012). The background task in
/// [`spawn`] is written against this trait, so it is the same for every way of reaching a display
/// and can be tested with a fake one. HDMI-CEC becomes a third implementation on the Pi.
pub trait DisplayPower: Send + Sync + 'static {
    /// Switches the display on or to standby; `Ok` once the display accepted the command.
    fn set(&self, action: PowerAction) -> impl Future<Output = Result<(), PowerError>> + Send;

    /// Asks the display for its power state.
    fn state(&self) -> impl Future<Output = Result<DisplayPowerState, PowerError>> + Send;

    /// False if there is nothing to talk to, so nothing to poll.
    fn configured(&self) -> bool {
        true
    }
}

/// No power control configured (the default; also a PC whose graphics card has no CEC).
pub struct NoDisplay;

impl DisplayPower for NoDisplay {
    async fn set(&self, _: PowerAction) -> Result<(), PowerError> {
        Err(PowerError::NotConfigured)
    }

    async fn state(&self) -> Result<DisplayPowerState, PowerError> {
        Err(PowerError::NotConfigured)
    }

    fn configured(&self) -> bool {
        false
    }
}

/// A projector with a network port, reached over PJLink class 1.
pub struct PjLinkDisplay {
    /// `host:port`.
    address: String,
    password: Option<String>,
}

impl PjLinkDisplay {
    /// `host` may carry a port; PJLink's own port is the default.
    pub fn new(host: &str, password: Option<String>) -> Self {
        let address = if host.contains(':') { host.to_owned() } else { format!("{host}:{PJLINK_PORT}") };
        PjLinkDisplay { address, password }
    }
}

impl DisplayPower for PjLinkDisplay {
    async fn set(&self, action: PowerAction) -> Result<(), PowerError> {
        match pjlink(&self.address, self.password.as_deref(), power_command(action)).await? {
            PowerReply::Accepted => Ok(()),
            PowerReply::State(_) => Err(PowerError::Failure("a state instead of OK".into())),
        }
    }

    async fn state(&self) -> Result<DisplayPowerState, PowerError> {
        match pjlink(&self.address, self.password.as_deref(), POWER_QUERY).await? {
            PowerReply::State(state) => Ok(state),
            PowerReply::Accepted => Err(PowerError::Failure("OK instead of a state".into())),
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

/// Starts the background task for the configured display; its state changes go to the actor.
pub fn spawn(config: &DisplayPowerConfig, state: StateHandle) -> PowerHandle {
    let (changes, mut inbox) = mpsc::channel(8);
    tokio::spawn(async move {
        while let Some(current) = inbox.recv().await {
            state.display_power_changed(current).await;
        }
    });
    match config {
        DisplayPowerConfig::None => watch(NoDisplay, changes),
        DisplayPowerConfig::Pjlink { host, password } => {
            watch(PjLinkDisplay::new(host, password.clone()), changes)
        }
    }
}

/// Runs commands for `display` and polls its state; every change (only changes) is sent to
/// `changes`. Failures show up as `Unavailable` with a reason.
fn watch<D: DisplayPower>(display: D, changes: mpsc::Sender<DisplayPowerState>) -> PowerHandle {
    let (jobs, mut inbox) = mpsc::channel::<Job>(8);
    tokio::spawn(async move {
        let mut last = None;
        let mut report = async |result: Result<DisplayPowerState, PowerError>| {
            let current = result
                .unwrap_or_else(|error| DisplayPowerState::Unavailable { reason: error.reason().to_owned() });
            if last.as_ref() != Some(&current) {
                last = Some(current.clone());
                let _ = changes.send(current).await;
            }
            last.clone()
        };
        let mut current = report(display.state().await).await;
        if !display.configured() {
            // Nothing to poll; keep answering commands with "not configured".
            while let Some((_, reply)) = inbox.recv().await {
                let _ = reply.send(Err(PowerError::NotConfigured));
            }
            return;
        }
        loop {
            let changing =
                matches!(current, Some(DisplayPowerState::WarmingUp | DisplayPowerState::CoolingDown));
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
            current = report(display.state().await).await;
        }
    });
    PowerHandle { jobs }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A display that answers from memory; `refuse` makes every command fail as busy.
    struct FakeDisplay {
        state: Mutex<DisplayPowerState>,
        refuse: bool,
    }

    impl DisplayPower for FakeDisplay {
        async fn set(&self, action: PowerAction) -> Result<(), PowerError> {
            if self.refuse {
                return Err(PowerError::Busy);
            }
            *self.state.lock().unwrap() = match action {
                PowerAction::On => DisplayPowerState::WarmingUp,
                PowerAction::Standby => DisplayPowerState::CoolingDown,
            };
            Ok(())
        }

        async fn state(&self) -> Result<DisplayPowerState, PowerError> {
            Ok(self.state.lock().unwrap().clone())
        }
    }

    fn fake(state: DisplayPowerState, refuse: bool) -> FakeDisplay {
        FakeDisplay { state: Mutex::new(state), refuse }
    }

    #[tokio::test]
    async fn a_command_is_answered_and_the_new_state_reported_at_once() {
        let (changes, mut reported) = mpsc::channel(8);
        let power = watch(fake(DisplayPowerState::Standby, false), changes);
        assert_eq!(reported.recv().await, Some(DisplayPowerState::Standby));
        power.run(PowerAction::On).await.unwrap();
        assert_eq!(reported.recv().await, Some(DisplayPowerState::WarmingUp), "no wait for the poll");
    }

    #[tokio::test]
    async fn a_refused_command_is_an_error_and_an_unchanged_state_is_not_reported_again() {
        let (changes, mut reported) = mpsc::channel(8);
        let power = watch(fake(DisplayPowerState::On, true), changes);
        assert_eq!(reported.recv().await, Some(DisplayPowerState::On));
        assert_eq!(power.run(PowerAction::Standby).await, Err(PowerError::Busy));
        assert!(reported.try_recv().is_err(), "still on; nothing new to report");
    }

    #[tokio::test]
    async fn without_a_display_commands_fail_and_the_state_says_why() {
        let (changes, mut reported) = mpsc::channel(8);
        let power = watch(NoDisplay, changes);
        let unavailable = DisplayPowerState::Unavailable { reason: "not_configured".into() };
        assert_eq!(reported.recv().await, Some(unavailable));
        assert_eq!(power.run(PowerAction::On).await, Err(PowerError::NotConfigured));
    }
}
