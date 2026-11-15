// SPDX-License-Identifier: GPL-3.0-or-later
//! Facts about the machine the server runs on, behind a trait so tests can inject them.

/// Platform facts the server depends on.
pub trait Platform: Send + Sync {
    /// Identifier of the current OS boot. Equal values across server restarts mean only the
    /// process restarted; a different value means the machine rebooted (ADR 0003).
    /// `None` when the platform cannot tell; the server then assumes a reboot.
    fn boot_id(&self) -> Option<String>;
}

/// The real platform.
pub struct SystemPlatform;

impl Platform for SystemPlatform {
    #[cfg(target_os = "linux")]
    fn boot_id(&self) -> Option<String> {
        let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        Some(id.trim().to_owned())
    }

    // Windows (pilot) and other systems: no boot id yet, so every start counts as a reboot and
    // the screen comes back with the logo. Safe, just less convenient after a process crash.
    #[cfg(not(target_os = "linux"))]
    fn boot_id(&self) -> Option<String> {
        None
    }
}

/// A platform with a fixed boot id, for tests.
pub struct FixedPlatform(pub Option<String>);

impl Platform for FixedPlatform {
    fn boot_id(&self) -> Option<String> {
        self.0.clone()
    }
}
