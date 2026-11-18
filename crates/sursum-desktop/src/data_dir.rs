// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the desktop program keeps its database: the per-user application data folder.

use std::path::PathBuf;

/// `%LOCALAPPDATA%\sursum` on Windows, `$XDG_DATA_HOME/sursum` (or `~/.local/share/sursum`)
/// elsewhere; `None` when the environment names no home at all.
pub fn default() -> Option<PathBuf> {
    let env = |name| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|home| home.join(".local/share")))?
    };
    Some(base.join("sursum"))
}
