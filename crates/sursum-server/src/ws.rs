// SPDX-License-Identifier: GPL-3.0-or-later
//! WebSocket sessions for the screen (read-only) and the remotes.
//!
//! A session waits for `Hello`, answers with `Welcome` and the full snapshot, then forwards
//! every new snapshot and (for remotes) executes commands. See `docs/protocol.md`.
//!
//! Rights are checked for every command, not only at connection time: admin mode expires and a
//! device can be revoked while its tablet stays connected. A revocation closes the session.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use axum::http::HeaderMap;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use sursum_protocol::{
    ClientId, ClientMsg, CmdOutcome, Command, ErrorCode, PROTOCOL_VERSION, Role, ServerMsg,
};
use tokio::sync::watch;

use crate::BUILD_ID;
use crate::actor::StateHandle;
use crate::auth::{Access, Rights};

/// The access rules and the upgrade request's headers (with the device cookie).
pub struct Guard {
    pub access: Arc<Access>,
    pub headers: HeaderMap,
    /// Display power commands go to their own task, not to the state actor (ADR 0012).
    pub power: crate::display_power::PowerHandle,
}

impl Guard {
    fn rights(&self) -> Rights {
        self.access.rights(&self.headers, Instant::now())
    }
}

/// Commands that change the device's setup rather than the service: admin mode only.
fn needs_admin(command: &Command) -> bool {
    matches!(command, Command::SetScreenProfile { .. } | Command::Calibrate { .. } | Command::SetLogo { .. })
}

/// A remote's right to send commands.
struct Control<'a> {
    state: &'a StateHandle,
    client_id: ClientId,
    guard: &'a Guard,
}

impl Control<'_> {
    /// Runs a command if this device may, else answers `Forbidden`.
    async fn run(&self, base_rev: sursum_protocol::Rev, cmd: Command) -> CmdOutcome {
        let required = if needs_admin(&cmd) { Rights::Admin } else { Rights::Operator };
        if self.guard.rights() < required {
            return CmdOutcome::Rejected { code: ErrorCode::Forbidden };
        }
        if let Command::DisplayPower { action } = cmd {
            return match self.guard.power.run(action).await {
                Ok(()) => CmdOutcome::Applied { rev: self.state.remote.borrow().rev },
                Err(error) => CmdOutcome::Rejected { code: error.code() },
            };
        }
        self.state.command(self.client_id.clone(), base_rev, cmd).await
    }
}

/// How long a client may take to send `Hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// A client that sends nothing (not even `Ping`) for this long is considered gone.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs one session until the client disconnects.
pub async fn session(socket: WebSocket, state: StateHandle, endpoint: Role, guard: Guard) {
    let (mut sender, mut receiver) = socket.split();
    let hello = tokio::time::timeout(HELLO_TIMEOUT, next_message(&mut receiver)).await;
    let Ok(Some(ClientMsg::Hello { protocol, role, client_id, .. })) = hello else {
        return;
    };
    if role != endpoint {
        return;
    }
    let welcome = ServerMsg::Welcome { protocol: PROTOCOL_VERSION, build_id: BUILD_ID.to_owned() };
    if send(&mut sender, &welcome).await.is_err() || protocol != PROTOCOL_VERSION {
        // A client with another protocol version reloads itself after reading `Welcome`.
        return;
    }
    tracing::debug!(%client_id, ?role, "client connected");
    let result = match role {
        Role::Screen => {
            forward_loop(
                &mut sender,
                &mut receiver,
                state.screen.clone(),
                |state| ServerMsg::Screen { state },
                None,
            )
            .await
        }
        Role::Remote => {
            let remote = state.remote.clone();
            let control = Control { state: &state, client_id: client_id.clone(), guard: &guard };
            forward_loop(
                &mut sender,
                &mut receiver,
                remote,
                |state| ServerMsg::Remote { state },
                Some(control),
            )
            .await
        }
    };
    tracing::debug!(%client_id, ?result, "client disconnected");
}

/// Sends the current snapshot, then forwards changes and handles client messages.
async fn forward_loop<T: Clone + Serialize>(
    sender: &mut SplitSink<WebSocket, Message>,
    receiver: &mut SplitStream<WebSocket>,
    mut snapshots: watch::Receiver<std::sync::Arc<T>>,
    wrap: impl Fn(T) -> ServerMsg,
    control: Option<Control<'_>>,
) -> Result<(), &'static str> {
    let first = snapshots.borrow_and_update().as_ref().clone();
    send(sender, &wrap(first)).await.map_err(|_| "send failed")?;
    let mut revocations = control.as_ref().map(|control| control.guard.access.revocations());
    // A revocation between the upgrade and this subscription would otherwise go unnoticed.
    if control.as_ref().is_some_and(|control| control.guard.rights() == Rights::None) {
        return Err("device revoked");
    }
    loop {
        tokio::select! {
            Some(()) = revoked(&mut revocations) => {
                if control.as_ref().is_some_and(|control| control.guard.rights() == Rights::None) {
                    return Err("device revoked");
                }
            }
            changed = snapshots.changed() => {
                changed.map_err(|_| "state actor stopped")?;
                let snapshot = snapshots.borrow_and_update().as_ref().clone();
                send(sender, &wrap(snapshot)).await.map_err(|_| "send failed")?;
            }
            message = tokio::time::timeout(IDLE_TIMEOUT, next_message(receiver)) => {
                let message = message.map_err(|_| "idle timeout")?.ok_or("closed")?;
                let reply = match (message, &control) {
                    (ClientMsg::Ping, _) => Some(ServerMsg::Pong),
                    (ClientMsg::Log { message }, _) => {
                        tracing::warn!(client_message = %message, "client reported a problem");
                        None
                    }
                    (ClientMsg::Cmd { seq, base_rev, cmd }, Some(control)) => {
                        let outcome = control.run(base_rev, cmd).await;
                        Some(ServerMsg::CmdResult { seq, outcome })
                    }
                    // The screen is read-only.
                    (ClientMsg::Cmd { seq, .. }, None) => {
                        Some(ServerMsg::CmdResult { seq, outcome: CmdOutcome::Rejected { code: ErrorCode::Forbidden } })
                    }
                    (ClientMsg::Hello { .. }, _) => None,
                };
                if let Some(reply) = reply {
                    send(sender, &reply).await.map_err(|_| "send failed")?;
                }
            }
        }
    }
}

/// Resolves when a device was revoked somewhere; never for sessions without a device (screen).
async fn revoked(revocations: &mut Option<tokio::sync::watch::Receiver<u64>>) -> Option<()> {
    match revocations {
        Some(receiver) => receiver.changed().await.ok(),
        None => std::future::pending().await,
    }
}

/// Reads the next protocol message, skipping pings/pongs and malformed frames.
/// Returns `None` when the connection closes.
async fn next_message(receiver: &mut SplitStream<WebSocket>) -> Option<ClientMsg> {
    while let Some(frame) = receiver.next().await {
        match frame.ok()? {
            Message::Text(text) => match serde_json::from_str(text.as_str()) {
                Ok(message) => return Some(message),
                Err(error) => tracing::warn!(%error, "ignoring a malformed client message"),
            },
            Message::Close(_) => return None,
            _ => {}
        }
    }
    None
}

async fn send(sender: &mut SplitSink<WebSocket, Message>, message: &ServerMsg) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).expect("protocol types always serialize");
    sender.send(Message::Text(json.into())).await
}

/// Cross-site WebSocket hijacking protection: browsers attach cookies to cross-origin WebSocket
/// upgrades, so the `Origin` must be this server (same host and port) or explicitly allowed.
/// Requests without `Origin` come from non-browser clients and are allowed.
pub fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) else {
        return true;
    };
    if allowed.iter().any(|candidate| candidate == origin) {
        return true;
    }
    let host = headers.get("host").and_then(|value| value.to_str().ok());
    let origin_host = origin.split_once("://").map(|(_, rest)| rest);
    matches!((origin_host, host), (Some(origin_host), Some(host)) if origin_host.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(origin: Option<&str>, host: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("host", host.parse().unwrap());
        if let Some(origin) = origin {
            headers.insert("origin", origin.parse().unwrap());
        }
        headers
    }

    #[test]
    fn same_origin_is_allowed() {
        assert!(origin_allowed(&headers(Some("http://192.168.1.10:8080"), "192.168.1.10:8080"), &[]));
    }

    #[test]
    fn foreign_origin_is_refused_unless_listed() {
        let foreign = headers(Some("http://evil.example"), "192.168.1.10:8080");
        assert!(!origin_allowed(&foreign, &[]));
        assert!(origin_allowed(&foreign, &["http://evil.example".to_owned()]));
    }

    #[test]
    fn missing_origin_is_allowed() {
        assert!(origin_allowed(&headers(None, "localhost:8080"), &[]));
    }
}
