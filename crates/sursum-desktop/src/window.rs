// SPDX-License-Identifier: GPL-3.0-or-later
//! The program's window: a web view showing the remote, drawn by the system's own engine. It has
//! no features of its own (ADR 0020): it shows the page, saves downloads, keeps the remote inside
//! and sends every other link to the system's browser.
//!
//! [`run`] calls back to start the server once it knows the window will open, then blocks the
//! main thread until the window is closed.

#[cfg(target_os = "linux")]
#[path = "window/linux.rs"]
mod platform;

#[cfg(windows)]
#[path = "window/windows.rs"]
mod platform;

pub use platform::run;

/// What the window shows.
pub enum Content {
    /// The remote, served at `origin` (`http://127.0.0.1:<port>`).
    Remote { origin: String },
    /// A page of its own instead (another copy runs, the server failed).
    Message(String),
}

impl Content {
    /// Where the remote starts.
    fn remote_url(origin: &str) -> String {
        format!("{origin}/remote/")
    }
}

/// True if `url` belongs to the server at `origin` and may load in the window; anything else
/// (the GNU and GitHub links on the licence page) opens in the browser instead.
fn stays_inside(origin: &str, url: &str) -> bool {
    url.strip_prefix(origin).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// True for links worth handing to the system's browser: web pages only, never files or other
/// programs' schemes.
fn is_web_link(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_own_server_stays_inside() {
        let origin = "http://127.0.0.1:41234";
        assert!(stays_inside(origin, "http://127.0.0.1:41234/remote/"));
        assert!(stays_inside(origin, "http://127.0.0.1:41234/api/library/export"));
        assert!(stays_inside(origin, "http://127.0.0.1:41234"));
        assert!(!stays_inside(origin, "http://127.0.0.1:412345/remote/"), "another port");
        assert!(!stays_inside(origin, "http://127.0.0.1:41234.evil.example/"));
        assert!(!stays_inside(origin, "https://github.com/dpieczynski/sursum"));
    }

    #[test]
    fn only_web_links_go_to_the_browser() {
        assert!(is_web_link("https://www.gnu.org/licenses/gpl-3.0.html"));
        assert!(!is_web_link("file:///etc/passwd") && !is_web_link("ms-settings:"));
    }
}
