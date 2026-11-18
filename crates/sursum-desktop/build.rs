// SPDX-License-Identifier: GPL-3.0-or-later
//! The icon and name of `sursum.exe` in Explorer, the taskbar and SmartScreen. Only with MSVC's
//! resource compiler, which the Windows release build has; the GNU cross build used for linting
//! goes without.

fn main() {
    println!("cargo:rerun-if-changed=assets/sursum.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }
    let mut resources = winresource::WindowsResource::new();
    resources.set_icon("assets/sursum.ico");
    resources.set("ProductName", "Sursum");
    resources.set("FileDescription", "Sursum");
    if let Err(error) = resources.compile() {
        panic!("compiling the Windows resources: {error}");
    }
}
