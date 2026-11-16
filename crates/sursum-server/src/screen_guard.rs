// SPDX-License-Identifier: GPL-3.0-or-later
//! Watching the screen's connection (plan §11.3). The kiosk browser keeps one WebSocket open; if
//! it is gone for longer than a grace period (a crashed renderer, a "sad tab", a stopped kiosk),
//! the server runs a configured command — on the device, restarting the kiosk unit. Remotes are
//! told whether a screen is connected, so the operator knows the slide is really up there.

use std::time::Duration;

use tokio::sync::watch;

use crate::actor::StateHandle;

/// `[screen_guard]` in `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenGuardConfig {
    /// The command and its arguments, e.g. `["systemctl", "restart", "sursum-kiosk"]`.
    pub restart_command: Vec<String>,
    /// How long the screen may be away before the command runs.
    #[serde(default = "default_grace_s")]
    pub grace_s: u64,
}

fn default_grace_s() -> u64 {
    5
}

/// After running the command, wait this long before trying again (the kiosk needs time to start).
const RETRY: Duration = Duration::from_secs(30);

/// How many screens are connected; shared by all screen sessions.
#[derive(Clone)]
pub struct ScreenPresence {
    count: watch::Sender<usize>,
}

impl Default for ScreenPresence {
    fn default() -> Self {
        ScreenPresence { count: watch::Sender::new(0) }
    }
}

impl ScreenPresence {
    /// Counts a screen session until the returned value is dropped.
    pub fn connected(&self) -> Connected {
        self.count.send_modify(|count| *count += 1);
        Connected { presence: self.clone() }
    }

    fn subscribe(&self) -> watch::Receiver<usize> {
        self.count.subscribe()
    }
}

/// One connected screen session; dropping it counts the screen as gone.
pub struct Connected {
    presence: ScreenPresence,
}

impl Drop for Connected {
    fn drop(&mut self) {
        self.presence.count.send_modify(|count| *count = count.saturating_sub(1));
    }
}

/// Reports presence changes to the state actor and, if configured, restarts the kiosk when the
/// screen stays away.
pub fn spawn(presence: &ScreenPresence, config: Option<ScreenGuardConfig>, state: StateHandle) {
    let mut count = presence.subscribe();
    tokio::spawn(async move {
        let mut was_connected = None;
        loop {
            let connected = *count.borrow_and_update() > 0;
            if was_connected != Some(connected) {
                state.screen_connected(connected).await;
                was_connected = Some(connected);
            }
            if connected || config.is_none() {
                if count.changed().await.is_err() {
                    return;
                }
                continue;
            }
            let Some(config) = &config else { continue };
            let grace = Duration::from_secs(config.grace_s);
            // Wait for the screen to come back within the grace period.
            let back =
                matches!(tokio::time::timeout(grace, count.wait_for(|count| *count > 0)).await, Ok(Ok(_)));
            if back {
                continue;
            }
            restart_kiosk(&config.restart_command).await;
            let _back = tokio::time::timeout(RETRY, count.wait_for(|count| *count > 0)).await.is_ok();
        }
    });
}

async fn restart_kiosk(command: &[String]) {
    let Some((program, arguments)) = command.split_first() else { return };
    tracing::warn!(?command, "the screen has been away too long; restarting the kiosk");
    let (program, arguments) = (program.clone(), arguments.to_vec());
    let status =
        tokio::task::spawn_blocking(move || std::process::Command::new(program).args(arguments).status())
            .await;
    match status {
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => tracing::error!(%status, "the kiosk restart command failed"),
        Ok(Err(error)) => tracing::error!(%error, "could not run the kiosk restart command"),
        Err(_) => tracing::error!("the kiosk restart task failed"),
    }
}
