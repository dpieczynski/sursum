// SPDX-License-Identifier: GPL-3.0-or-later
//! Server configuration: command-line flags plus an optional `config.toml` in the data directory.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Deserialize;

/// Effective configuration of one server instance.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to listen on.
    pub listen: SocketAddr,
    /// Directory for the database, backups and assets (`/var/lib/sursum` on the device).
    pub data_dir: PathBuf,
    /// Extra `Origin` values allowed to open WebSockets (e.g. Trunk dev servers).
    pub allowed_origins: Vec<String>,
    /// Host names the server answers to besides IP addresses, `localhost` and `*.local`
    /// (protection against DNS rebinding), e.g. a name in the parish router's DNS.
    pub allowed_hosts: Vec<String>,
    /// Crossfade duration in milliseconds; 0 for instant cuts.
    pub transition_ms: u32,
    /// Accept connections from any address, not only private networks (for special setups).
    pub allow_public_clients: bool,
    /// Who may use the server (never from `config.toml`): the PIN on the device, or no PIN on a
    /// loopback address (the desktop program, development and tests).
    pub access: crate::auth::AccessMode,
    /// What kind of installation this is, so the remote offers only what makes sense: a device
    /// with a screen, or the desktop program's library without one (never from `config.toml`).
    pub installation: sursum_protocol::Installation,
    /// Test-only: delay serving WebAssembly by this many milliseconds (`e2e-hooks` feature).
    pub wasm_delay_ms: u64,
    /// The protocol version the WebSocket sessions speak: always `PROTOCOL_VERSION`, except in
    /// browser tests (`e2e-hooks` feature) that play a server from another release.
    pub protocol_version: u16,
    /// How the server switches the display on and off (ADR 0012).
    pub display_power: DisplayPowerConfig,
    /// What to do when the screen stays disconnected (restart the kiosk); off when `None`.
    pub screen_guard: Option<crate::screen_guard::ScreenGuardConfig>,
    /// The key system updates must be signed with: the one compiled in from
    /// `SURSUM_BUNDLE_KEY`. Never read from `config.toml`; tests set their own.
    pub update_key: Option<ed25519_dalek::VerifyingKey>,
    /// The running system's version file and the file that asks the root unit to install a
    /// system update (ADR 0019); the device's paths, never read from `config.toml`; tests set
    /// their own.
    pub system_release: PathBuf,
    pub system_request: PathBuf,
}

/// `[display_power]` in `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case", deny_unknown_fields)]
pub enum DisplayPowerConfig {
    /// No power control (the remote hides its buttons).
    #[default]
    None,
    /// A projector with a LAN port. `host` is `address` or `address:port` (port 4352 by
    /// default); `password` only if one is set on the projector. Kept in `config.toml` on the
    /// device only, never in the database or the repository.
    Pjlink { host: String, password: Option<String> },
    /// A TV (or projector) on the Pi's HDMI0 port, over HDMI-CEC through the kernel's CEC device.
    /// `device` only if it is not `/dev/cec0`. Linux only; elsewhere it reports "not configured".
    Cec { device: Option<std::path::PathBuf> },
}

impl Config {
    /// Defaults for a data directory, before reading `config.toml`.
    pub fn with_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        Config {
            listen: SocketAddr::from(([0, 0, 0, 0], 8080)),
            data_dir: data_dir.into(),
            allowed_origins: Vec::new(),
            allowed_hosts: Vec::new(),
            transition_ms: 250,
            allow_public_clients: false,
            access: crate::auth::AccessMode::Pin,
            installation: sursum_protocol::Installation::Screen,
            wasm_delay_ms: 0,
            protocol_version: sursum_protocol::PROTOCOL_VERSION,
            display_power: DisplayPowerConfig::None,
            screen_guard: None,
            update_key: crate::signing::trusted_key(),
            system_release: crate::system_update::SYSTEM_RELEASE.into(),
            system_request: crate::system_update::REQUEST.into(),
        }
    }

    /// Applies `config.toml` from the data directory, if it exists.
    pub fn load_file(self) -> anyhow::Result<Self> {
        self.load_files(None)
    }

    /// Applies the system's `config.toml` (on the device `/etc/sursum/config.toml`, part of the
    /// read-only image, so system updates can change its defaults), then the data directory's
    /// (this device's own settings, e.g. a projector's password), which wins setting by setting.
    /// Either may be missing.
    pub fn load_files(mut self, system: Option<&Path>) -> anyhow::Result<Self> {
        if let Some(system) = system
            && let Some(file) = read_file(system)?
        {
            self = self.apply(file);
        }
        let path = self.data_dir.join("config.toml");
        Ok(match read_file(&path)? {
            Some(file) => self.apply(file),
            None => self,
        })
    }

    fn apply(mut self, file: FileConfig) -> Self {
        if let Some(listen) = file.listen {
            self.listen = listen;
        }
        if let Some(origins) = file.allowed_origins {
            self.allowed_origins = origins;
        }
        if let Some(hosts) = file.allowed_hosts {
            self.allowed_hosts = hosts;
        }
        if file.screen_profile.is_some() {
            // Read before 2026-11-19; the calibration panel is the only source now.
            tracing::warn!(
                "config.toml: [screen_profile] is ignored; set the screen in the remote's calibration panel"
            );
        }
        if let Some(transition_ms) = file.transition_ms {
            self.transition_ms = transition_ms;
        }
        if let Some(allow) = file.allow_public_clients {
            self.allow_public_clients = allow;
        }
        if let Some(display_power) = file.display_power {
            self.display_power = display_power;
        }
        if let Some(screen_guard) = file.screen_guard {
            self.screen_guard = Some(screen_guard);
        }
        self
    }
}

/// The subset of settings `config.toml` may contain.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    listen: Option<SocketAddr>,
    allowed_origins: Option<Vec<String>>,
    allowed_hosts: Option<Vec<String>>,
    /// No longer read (the calibration panel stores the profile); accepted so an old file
    /// does not stop the server.
    screen_profile: Option<toml::Table>,
    transition_ms: Option<u32>,
    allow_public_clients: Option<bool>,
    display_power: Option<DisplayPowerConfig>,
    screen_guard: Option<crate::screen_guard::ScreenGuardConfig>,
}

fn read_file(path: &Path) -> anyhow::Result<Option<FileConfig>> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map(Some).with_context(|| format!("parsing {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_keeps_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_data_dir(dir.path()).load_file().unwrap();
        assert_eq!(config.transition_ms, 250);
    }

    #[test]
    fn pjlink_projector_is_configured_in_its_own_table() {
        let dir = tempfile::tempdir().unwrap();
        let text = "[display_power]\nbackend = \"pjlink\"\nhost = \"192.168.1.50\"\npassword = \"tajne\"\n";
        std::fs::write(dir.path().join("config.toml"), text).unwrap();
        let config = Config::with_data_dir(dir.path()).load_file().unwrap();
        assert_eq!(
            config.display_power,
            DisplayPowerConfig::Pjlink { host: "192.168.1.50".into(), password: Some("tajne".into()) }
        );
    }

    #[test]
    fn cec_tv_is_configured_without_a_device_by_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "[display_power]\nbackend = \"cec\"\n").unwrap();
        let config = Config::with_data_dir(dir.path()).load_file().unwrap();
        assert_eq!(config.display_power, DisplayPowerConfig::Cec { device: None });
    }

    #[test]
    fn the_device_file_wins_over_the_system_file_setting_by_setting() {
        let system = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            system.path(),
            "transition_ms = 400\n[display_power]\nbackend = \"cec\"\n\
             [screen_guard]\nrestart_command = [\"systemctl\", \"restart\", \"sursum-kiosk\"]\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = Config::with_data_dir(dir.path()).load_files(Some(system.path())).unwrap();
        assert_eq!(
            (config.transition_ms, config.display_power.clone()),
            (400, DisplayPowerConfig::Cec { device: None })
        );
        assert_eq!(config.screen_guard.unwrap().restart_command, ["systemctl", "restart", "sursum-kiosk"]);

        let device = "[display_power]\nbackend = \"pjlink\"\nhost = \"10.0.0.9\"\n";
        std::fs::write(dir.path().join("config.toml"), device).unwrap();
        let config = Config::with_data_dir(dir.path()).load_files(Some(system.path())).unwrap();
        assert_eq!(config.transition_ms, 400, "kept from the system file");
        assert_eq!(
            config.display_power,
            DisplayPowerConfig::Pjlink { host: "10.0.0.9".into(), password: None }
        );
        assert!(config.screen_guard.is_some(), "kept from the system file");

        let missing = dir.path().join("no-such.toml");
        assert!(Config::with_data_dir(dir.path()).load_files(Some(&missing)).is_ok());
    }

    #[test]
    fn file_overrides_defaults_and_an_old_screen_profile_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            r#"
listen = "127.0.0.1:9000"
transition_ms = 0

[screen_profile]
safe_area = { top = 10, right = 10, bottom = 10, left = 10 }
"#,
        )
        .unwrap();
        let config = Config::with_data_dir(dir.path()).load_file().unwrap();
        assert_eq!(config.listen.port(), 9000);
        assert_eq!(config.transition_ms, 0);
    }

    #[test]
    fn unknown_keys_are_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "colour = 'red'\n").unwrap();
        assert!(Config::with_data_dir(dir.path()).load_file().is_err());
    }
}
