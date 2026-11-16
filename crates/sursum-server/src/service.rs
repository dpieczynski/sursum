// SPDX-License-Identifier: GPL-3.0-or-later
//! Talking to systemd (`Type=notify`, `WatchdogSec=`): "ready" once the server listens, and a
//! watchdog ping only while the state actor still answers. A hung actor then gets the service
//! restarted although the process is alive — and the screen keeps its slide meanwhile.
//!
//! The protocol is one datagram per message to the socket in `$NOTIFY_SOCKET` (see
//! `sd_notify(3)`); outside systemd (development, Windows) nothing is sent.

use std::time::Duration;

/// Tells systemd the server is up (no-op outside systemd).
pub fn ready() {
    notify("READY=1");
}

/// How often to ping the watchdog: half of `WatchdogSec`, if systemd asked for pings.
pub fn watchdog_interval() -> Option<Duration> {
    let usec = std::env::var("WATCHDOG_USEC").ok()?;
    let pid = std::env::var("WATCHDOG_PID").ok();
    interval_from(&usec, pid.as_deref(), std::process::id())
}

/// Pings the watchdog every [`watchdog_interval`] while `alive` says the server works.
pub fn spawn_watchdog<F, Fut>(alive: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send,
{
    let Some(interval) = watchdog_interval() else { return };
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if tokio::time::timeout(interval, alive()).await.unwrap_or(false) {
                notify("WATCHDOG=1");
            } else {
                tracing::error!("the state actor does not answer; withholding the watchdog ping");
            }
        }
    });
}

/// `WATCHDOG_USEC` (and `WATCHDOG_PID`, if set, must be us) → half the period.
fn interval_from(usec: &str, pid: Option<&str>, own_pid: u32) -> Option<Duration> {
    if pid.is_some_and(|pid| pid.parse::<u32>().ok() != Some(own_pid)) {
        return None;
    }
    let usec: u64 = usec.parse().ok().filter(|usec| *usec > 0)?;
    Some(Duration::from_micros(usec / 2))
}

fn notify(message: &str) {
    if let Some(path) = std::env::var_os("NOTIFY_SOCKET")
        && let Err(error) = send(&path, message)
    {
        tracing::warn!(%error, "could not notify systemd");
    }
}

#[cfg(target_os = "linux")]
fn send(path: &std::ffi::OsStr, message: &str) -> std::io::Result<()> {
    use std::os::linux::net::SocketAddrExt as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::net::{SocketAddr, UnixDatagram};
    let bytes = path.as_bytes();
    // A leading '@' names a socket in the abstract namespace.
    let address = match bytes.strip_prefix(b"@") {
        Some(name) => SocketAddr::from_abstract_name(name)?,
        None => SocketAddr::from_pathname(path)?,
    };
    UnixDatagram::unbound()?.send_to_addr(message.as_bytes(), &address)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn send(_: &std::ffi::OsStr, _: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watchdog_is_pinged_twice_per_period_and_only_for_us() {
        assert_eq!(interval_from("10000000", None, 7), Some(Duration::from_secs(5)));
        assert_eq!(interval_from("10000000", Some("7"), 7), Some(Duration::from_secs(5)));
        assert_eq!(interval_from("10000000", Some("8"), 7), None, "meant for another process");
        assert_eq!(interval_from("0", None, 7), None);
        assert_eq!(interval_from("soon", None, 7), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn messages_arrive_as_datagrams() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notify.sock");
        let receiver = std::os::unix::net::UnixDatagram::bind(&path).unwrap();
        send(path.as_os_str(), "READY=1").unwrap();
        let mut buffer = [0_u8; 64];
        let length = receiver.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..length], b"READY=1");
    }
}
