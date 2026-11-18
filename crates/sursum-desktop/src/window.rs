// SPDX-License-Identifier: GPL-3.0-or-later
//! The program's window: a web view showing the remote, drawn by the system's own engine.
//! [`run`] calls back to start the server once it knows the window will open, then blocks the
//! main thread until the window is closed.

#[cfg(target_os = "linux")]
#[path = "window/linux.rs"]
mod platform;

#[cfg(windows)]
#[path = "window/windows.rs"]
mod platform;

pub use platform::run;
