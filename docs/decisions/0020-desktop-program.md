# 0020 — Desktop program: the same server in a window of its own

Status: Accepted (2026-11-18). Points marked ⚠ wait for the first run on Windows. ADR 0023
(2026-11-19) settles two open points: no projector mode on a parish PC and no macOS build.

## Context
The device in the church is offline, so nobody can add or correct songs from home. The pieces
for doing it at home exist (the editor, the `.sursum` export and merge), but the only way to run
them on a PC is `sursum-server` plus a browser tab: a console window, a tab in whatever browser
is the default, a process that keeps running after the tab is closed, a port that is taken on
the second start. A parishioner should get an ordinary program. The parish PC of the Windows
pilot (P1) has the same problem with three processes started by a PowerShell script.

A spike (branch `spike/desktop-shell`, 2026-11-18) showed that a window around the existing
server is small: about 100 lines per platform pair, no new logic.

## Decision
1. **A new binary `sursum`** (crate `sursum-desktop`; `sursum.exe` on Windows) starts the
   `sursum-server` library in its own process, listening on `127.0.0.1` on a free port, and shows
   the remote in a window titled "Sursum". Closing the window stops the server. Nothing outside
   the computer can reach it, so it runs without a PIN. Data lives in the user's application
   data folder (`%LOCALAPPDATA%\sursum`, `$XDG_DATA_HOME/sursum`).
2. **Hard rule 1 is untouched**: the server still holds all logic, the pages still only draw. The
   desktop program is a way to start and show them, nothing more. The device keeps
   `sursum-server`.
3. **The window has no features.** It shows the page, saves a downloaded file and keeps one copy
   of the program running. Anything that can be done in the remote or in the server is done
   there. This keeps the code that differs between platforms at its present size.
4. **One window module per platform** behind a single function (`window::run`):
   - Windows: `tao` + `wry`, drawing with WebView2 (the Edge engine that ships with Windows; the
     same Blink engine as the tablets and the device's kiosk).
   - Linux: `gtk4` + `webkit6` (WebKitGTK 6.0). `wry` on Linux needs WebKitGTK 4.1 on GTK 3,
     which current distributions no longer install by default (Fedora 45 ships only 6.0) and
     which pins glib 0.18.
5. **Two levels of support.**
   - *Windows is the product*: tests, releases, the operator guide.
   - *Linux is best effort*: built in CI with one smoke test (starts, the remote connects, the
     editor opens); no promise that the preview matches the screen to the pixel, because the
     remote runs on WebKit there. If it starts to cost too much, the Linux window is deleted and
     Linux goes back to a browser tab; Windows is not affected.

## Still to decide (after the first run on Windows)
- ⚠ **One file or two on Windows.** The GNU target (our cross build with `cargo zigbuild`) links
  WebView2 dynamically, so `WebView2Loader.dll` must lie next to `sursum.exe`; the MSVC target
  links it statically but needs a Windows runner in CI. The DLL's license must be checked before
  it is shipped.
- ⚠ WebView2 on older Windows 10 installations; the SmartScreen warning for an unsigned program.
- What the remote shows at home, where there is no screen (a library-only view), and whether the
  same program later opens the screen on the projector of a parish PC, replacing
  `deploy/windows/start-pilot.ps1`.
- How the Linux program is distributed (it links the system's WebKitGTK 6.0).

## Consequences
- `cargo deny` cannot see that `tao`'s GTK 3 dependencies are never compiled (we use `tao` on
  Windows only): one advisory exception (RUSTSEC-2024-0370) with that reason in `deny.toml`.
- The devcontainer and CI need the GTK 4 and WebKitGTK 6.0 development packages; without them
  `cargo clippy --workspace` fails on Linux.
- The web view keeps its own data (storage, cache); it must get a folder of its own beside the
  database, not the same one (seen in the spike).
- The remote now also runs on WebKit. ADR 0015 binds only the screen to the WebKit API contract;
  the iPad plan in the roadmap extends the tests, and later the contract, to the remote.

## Rejected
- **A browser tab** opened by the server: not a program (see Context).
- **Edge in `--app` mode**: no new dependencies, but the window's icon and behavior belong to
  Edge, which can be uninstalled in the EU. Kept as the fallback if WebView2 through `wry` fails
  on Windows.
- **Tauri**: `wry` + `tao` plus its own IPC and build tooling; our pages already talk to the
  server over HTTP and WebSocket.
- **The domain logic compiled to WebAssembly** in a static page: breaks hard rule 1 and keeps
  the songs in browser storage.
- **Bundling Chromium (CEF)** to have one engine everywhere: 150 to 200 MB and a heavy build for
  a best-effort platform.

## Amendment (2026-11-18): one file, unsigned
- **Windows gets one file**: the MSVC target, built on a Windows runner (CI job
  `desktop-windows`, and in the release workflow), links the WebView2 loader statically. The
  loader comes from Microsoft's WebView2 SDK under the BSD 3-Clause license (checked in the SDK
  package); its text is on `/licenses`. The GNU cross build stays for `just lint`.
- **Not code-signed** for now: the operator guide shows how to get past the SmartScreen warning.
  Signing can come later without changing anything else.
- **Access**: the program runs the server in a named *local owner* mode (`AccessMode::LocalOwner`)
  that the server refuses on anything but a loopback address. Other users of the same computer can
  reach the port; that is accepted (they could read the data folder anyway). Web pages in the
  user's browser cannot: unknown `Host` names are refused, WebSockets check `Origin`, and JSON and
  upload requests need headers that force a CORS preflight.
- **iPad**: minimum iPadOS 16.4; the repository is private, so the iPad Simulator job runs by
  hand or weekly, not on every pull request (roadmap P2).

## Amendment (2026-11-20): one access mode without a PIN

The development mode (`--open-without-pin`) and the local owner mode were two ways of saying "no
PIN, everything allowed"; they differed only in that development allowed any address and a PIN set
later. They are now one mode, `AccessMode::Local`, with the local owner's rules: loopback only
(refused otherwise), never a PIN. `just dev` and the tests already listen on loopback; trying a
real tablet against a development build means pairing it with a PIN like any device.
