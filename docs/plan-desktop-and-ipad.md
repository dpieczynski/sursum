# Implementation plan: desktop program (D1) and iPad remote (P2)

Written 2026-11-18 from the spike on branch `spike/desktop-shell`. **Status (same day):** D1.1,
D1.3 to D1.8 and P2.1 to P2.6 are implemented as far as they can be without Windows, an iPad or a
GitHub run; what is left is in the roadmap's D1 and P2 sections (marked ⚠ or unchecked). The
findings below still hold; section 3 describes the spike as it was. The decisions are in
[ADR 0020](decisions/0020-desktop-program.md); the checkboxes are in
[roadmap.md](roadmap.md) (sections D1 and P2). Where this plan and those two differ, they win.
⚠ marks something nobody has verified yet: check it before relying on it.

## 1. What you are building

- **D1**: a program called `sursum` (`sursum.exe`) that a parishioner starts at home like any
  other program. It runs the same server as the church device, only on `127.0.0.1`, and shows
  the remote in its own window. They write songs, export the library (`.sursum`), and the church
  device merges the file.
- **P2**: the existing remote working on an iPad (Safari, WebKit), prepared and tested without
  owning one, confirmed once on a real device.

D1 comes first. P2 does not depend on D1, except that both make the remote run on WebKit.

## 2. Rules that matter here

- **The window has no features** (ADR 0020). It shows the page, saves a download and keeps one
  copy running. If something can be done in the remote or the server, do it there. Before adding
  code under `crates/sursum-desktop/src/window/`, ask whether it could live elsewhere.
- **All logic stays in the server** (hard rule 1). The desktop program decides nothing about
  songs, sets or what the remote shows; the server tells the remote.
- **No `unsafe`** (workspace lint `unsafe_code = "forbid"`). Win32 and GTK calls go through
  crates, never through our own FFI.
- **Windows is the product, Linux is best effort.** Do not hold a Windows release for a Linux
  problem, and do not promise a pixel-identical preview on Linux.
- UI text is Polish and lives in one place per crate (the remote uses
  `apps/remote/src/messages.rs`); code, comments and docs are English.

## 3. Where the spike stands

Nothing is committed. The branch holds:

| File | State |
|---|---|
| `crates/sursum-desktop/` (177 lines) | builds and runs on Linux; cross-compiles for Windows; never run on Windows |
| `Cargo.toml` | `sursum-server` added to `[workspace.dependencies]` |
| `deny.toml` | one advisory exception (RUSTSEC-2024-0370) |
| `docs/decisions/0020-…`, `docs/roadmap.md` | ADR and the D1/P2 sections |

Checked: Linux build and run on Fedora 45 (KDE, Wayland), one stable WebSocket connection from
the window; Windows cross-build with `cargo zigbuild` (9.9 MB, GUI subsystem); clippy
`-D warnings` for both targets; `cargo deny check`; SPDX headers.

Not checked: anything on Windows at run time; the export and merge dialogs on either platform;
the second-start behavior on Linux; what the remote looks like in WebKit.

## 4. Findings you must know before touching the code

1. **Build ids must match.** The server and the web apps compare `SURSUM_BUILD_ID`; a mismatch
   makes the page reload in a loop (it looks like a blank, flickering window). Always build
   through `just`, or export the same id for Trunk and for cargo. `apps/*/dist` on the spike
   machine was built with `SURSUM_BUILD_ID=spike-desktop`.
2. **`gtk4` needs the `v4_10` feature.** `webkit6` 0.6.1 uses `gtk::Accessible`, which `gtk4`
   0.11.5 hides behind that feature without `webkit6` enabling it. We need it anyway for
   `FileDialog`. Debian trixie has GTK 4.18.6 and WebKitGTK 2.54.0 (checked); Ubuntu 24.04 has
   GTK 4.14 ⚠.
3. **Linux needs development packages** to build, lint and test the crate:
   `libgtk-4-dev` and `libwebkitgtk-6.0-dev` (Debian names, checked on trixie). The crate is a
   workspace member (`crates/*`), so on a machine without them `cargo clippy --workspace` and
   `cargo test --workspace` (`just lint`, `just test`) fail. CI runs on `ubuntu-24.04` directly,
   not in the devcontainer.
4. **`tao` pulls GTK 3 crates into `Cargo.lock`** through its Linux-only dependencies. They are
   never compiled (we use `tao` on Windows only), but `cargo deny` does not follow target
   conditions along a path, hence the exception. Limiting `[graph] targets` does not help
   (tried).
5. **On Windows the GNU target needs `WebView2Loader.dll` next to the program.**
   `webview2-com-sys` links the loader statically only for MSVC. The DLL ships inside that crate
   (`x64/WebView2Loader.dll`). Its license is unchecked ⚠.
6. **The web view writes its own data.** On Linux WebKit put `storage/` and `mediakeys/` into
   the same folder as `sursum.db`. On Windows WebView2 by default creates its folder next to the
   executable ⚠ (`wry` passes an empty path unless a `WebContext` with a data directory is
   given), which fails in a read-only install location.
7. **The spike borrows the development switch `open_without_pin`.** Its meaning ("every device
   on the local network is admin") and its log warning are wrong for a program that listens on
   loopback only. See work package D1.5.
8. **What already protects a loopback server**: `known_host` refuses unknown `Host` names (DNS
   rebinding), WebSockets check `Origin` against `Host`, JSON endpoints need
   `Content-Type: application/json` and uploads need the `X-Sursum-Upload` header (both force a
   CORS preflight, so a web page in the user's browser cannot post to them). Other users of the
   same computer can reach the port; that is accepted and should be written into the ADR.
9. **With the GUI subsystem there is no console on Windows.** Log lines and startup errors go
   nowhere unless the program writes them to a file and shows failures in the window.
10. **The remote e2e tests pin Chromium** (`with_engine(Browser::Chromium, …)` in
    `e2e/tests/remote*.rs`); only the screen tests follow `SURSUM_E2E_BROWSER`. WPEWebDriver
    allows one session at a time.
11. **Safari findings from reading the remote** (not from running it): `index.html` sets
    `viewport-fit=cover` but no CSS uses `env(safe-area-inset-*)`; `navigator.vibrate` is already
    feature-detected (`apps/remote/src/prefs.rs`); `-webkit-touch-callout: none` and
    `user-select: none` are set in `style/base.css`; `dvh` is used (Safari 15.4+); full screen is
    requested in `apps/remote/src/settings/device.rs` without checking that the API exists.
12. **Do not screenshot the maintainer's desktop** to check the window. Use logs, the e2e
    harness (`just screenshots`, `just remote-sizes`), or ask.

## 5. D1 work packages, in order

Each package is one pull request. "Check" is what proves it; add tests where the package says so.

### D1.1 Land the spike so everyone can build it
- Commit the spike as it is (it is small and reviewed by the lints).
- Add `libgtk-4-dev libwebkitgtk-6.0-dev` to `.devcontainer/Containerfile` and to the CI jobs
  that run `just lint` and `just test`.
- `justfile`: `desktop` (build-web, then `cargo build --release -p sursum-desktop`) and
  `cross-win-desktop` (`cargo zigbuild --release -p sursum-desktop --target
  x86_64-pc-windows-gnu`); both inherit `SURSUM_BUILD_ID` from the justfile. Add the Windows
  build to the cross-compile CI job and upload `sursum.exe` with the DLL as an artifact.
- Clippy for the Windows target cannot run with plain cargo on Linux (`libsqlite3-sys` needs a
  C compiler for the target); `cargo-zigbuild clippy -p sursum-desktop --target
  x86_64-pc-windows-gnu -- -D warnings` works. Add it to `just lint`.
- Third-party notices: `just licenses` covers the server and the remote; add the desktop crate's
  manifest so `tao`, `wry`, `gtk4`, `webkit6` and their dependencies are listed.
- **Check:** CI green on all jobs; the artifact exists.

### D1.2 First run on Windows (needs the maintainer or a Windows machine)
Run the CI artifact on Windows 11 and, if possible, on the parish PC's Windows 10:
1. the window opens, shows the remote, no console window appears;
2. "Eksport bazy" saves `biblioteka.sursum` (WebView2's own download prompt);
3. "Scal bazę z pliku" opens a file picker and merges;
4. closing the window ends the process (Task Manager) and the next start opens the same data;
5. where WebView2 put its folder (finding 6), and whether WebView2 is present at all;
6. what SmartScreen says.

Write the results into the roadmap. If WebView2 through `wry` fails in a way that cannot be
fixed, the fallback in ADR 0020 is an Edge `--app` window; stop and ask before switching.

### D1.3 One file or two on Windows
Decide between the GNU target with `WebView2Loader.dll` beside the program (zip of two files,
built on Linux as today) and the MSVC target (one file, needs a Windows runner, the web apps
built there or passed as an artifact). Read the license shipped with Microsoft's WebView2 SDK
before redistributing the DLL. Record the choice as an amendment to ADR 0020, then update the
release workflow (`.github/workflows/release.yml`, `docs/releasing.md`).

### D1.4 Make the window behave like a program
All inside `crates/sursum-desktop`; keep `window::run` as the only platform seam.
- **Web view data in its own folder**: `<data dir>/webview`. Linux: build the `WebView` with a
  `webkit6::NetworkSession::new(Some(data), Some(cache))`. Windows:
  `wry::WebContext::new(Some(path))` and `WebViewBuilder::new_with_web_context`.
- **Log file** in the data directory (a plain writer for `tracing_subscriber`; keep the last
  run or two, not a growing file).
- **Failures are visible**: if the server cannot start (database locked, disk full), show a
  short Polish message in the window instead of exiting silently (`with_html` in `wry`,
  `load_html` in WebKit). Put these few strings in one `messages.rs`.
- **One copy at a time on Windows**: Linux already has it through the GTK application id. On
  Windows hold a lock file in the data directory with `std::fs::File::try_lock` (works on our
  toolchain, checked); the second copy shows the "already running" message page and exits. Do
  not add inter-process messaging just to raise the first window.
- **Stay on our own page**: refuse navigation and new windows that leave
  `http://127.0.0.1:<port>/` (`with_navigation_handler` and `with_new_window_req_handler` in
  `wry`; `decide-policy` in WebKit). The license page links to GitHub; those links must not
  replace the remote inside the window.
- **Icon**: window icon at run time (`tao::window::Icon::from_rgba`; on Linux a `.desktop`
  file and icon matching the application id `io.github.dpieczynski.Sursum`). The icon of the
  `.exe` file itself needs a resource; do that together with D1.3.
- **Check:** by hand on both platforms; unit tests for anything pure (paths, the lock).

### D1.5 A named access mode instead of `open_without_pin` (server)
`Access` takes a boolean that means "development: everyone is admin". The desktop program needs
"this computer's owner is admin" as a real, documented mode. Replace the boolean with an enum
(for example `PinPolicy { Required, OpenForDevelopment, LocalOwner }`) in
`crates/sursum-server/src/auth.rs` and `config.rs`; `LocalOwner` is only valid with a loopback
listen address (refuse to start otherwise) and logs its own message. Never readable from
`config.toml`. Tests next to the existing ones in `auth.rs` and `crates/sursum-server/tests`.

### D1.6 Library view: the remote on a computer without a screen
At home there is no screen, so the Msza view, the "screen is not connected" banner, the display
power and calibration settings make no sense.
- `sursum-protocol`: an enum in `SessionInfo` saying what kind of installation this is (a device
  with a screen, or a library on a desk), `#[serde(default)]` to the device. Data only; review
  the JSON snapshots.
- Server: set from the same configuration as D1.5.
- Remote (`apps/remote/src/shell.rs`, `settings/`): in the library kind show only the Zestaw
  view's song work (search, new song, editor) and the library and backup parts of Ustawienia;
  hide the Msza tab, the live strip, the screen banner and screen settings. The server keeps
  accepting the same commands; this is about what is offered, not about rights.
- **Check:** an e2e test in `e2e/tests` starting the server in the library mode; screenshots
  through `just remote-sizes`.

### D1.7 Linux by hand and one smoke test
Click through the "save as" dialog of the export, the file picker of the merge and a second
start (it should raise the first window). Add a smoke test that starts `sursum` with a temporary
`XDG_DATA_HOME`, waits for the remote to connect and stops it; it needs a display, so run it
under a headless compositor or mark it for manual runs if that proves fragile.

### D1.8 Release and guide
Release workflow attaches the Windows program; `docs/operator-guide-pl.md` gets "Pieśni
przygotowane w domu" (download, the SmartScreen warning, write, export, merge in the church);
README status line.

## 6. P2 work packages, in order

### P2.1 Remote tests on WebKit on Linux
Let the remote's engine come from the environment (for example `SURSUM_E2E_REMOTE_BROWSER`),
keeping the screen on Chromium in those tests, because WPEWebDriver gives one session. Tests
that need two remotes or CDP stay on Chromium. Start as a non-blocking CI step; fix what fails;
make it blocking. ⚠ whether WebDriver touch and pointer actions work in headless `cog`.

### P2.2 Fix what the audit found
- Safe areas: padding from `env(safe-area-inset-*)` on the shell's outer regions.
- Full screen: offer it only where `requestFullscreen` exists.
- Every text field at 16 px or more (Safari zooms the page otherwise).
- The live strip with the on-screen keyboard open.
- Add iPad sizes (768×1024, 1024×768, 820×1180) to the `remote_sizes` example.

### P2.3 Minimum version
Proposed iPadOS 16.4 ⚠ (ask the maintainer which iPads exist; an iPad Air 2 stops at iPadOS 15).
List the WebAssembly features in the built `.wasm` and compare with that Safari version.

### P2.4 iPad Simulator in CI
A macOS job: build the server natively (the Linux-only parts are already behind `cfg`; whether
the rest compiles on macOS is unchecked ⚠), start `safaridriver`, add a `Browser` variant with
`platformName: iOS`, `safari:useSimulator: true`, `safari:deviceType: iPad`, run the remote
subset and keep screenshots as artifacts. `continue-on-error` at first. ⚠ `safaridriver
--enable` on hosted runners; simulator sessions are known to be flaky; macOS minutes cost more
on a private repository.

### P2.5 The remote joins the WebKit contract
A new ADR extending ADR 0015's API contract to the remote; update the wording in
`docs/plan-2026-11.md` ("iPad: furtka").

### P2.6 One session on a real device
Add the checklist to `docs/testing.md`: pairing through `sursum.local` on the church network,
hold buttons, typing in the editor, lock and unlock, switching apps and coming back (the remote
reconnects), a whole set by touch and by the pedal, export and merge through the Files app.
Until Screen Wake Lock is available (it needs HTTPS, M6) the guide tells people to set Auto-Lock
to "Never" during the service. Only after this session does the guide say the iPad is supported.

## 7. What you need from the maintainer

| Needed | For |
|---|---|
| A run on Windows 11 and on the parish PC | D1.2 |
| Choice: two files from Linux CI, or one file from a Windows runner | D1.3 |
| Whether code signing is wanted now or the SmartScreen note in the guide is enough | D1.8 |
| Which iPads the parishes have; whether the repository is public | P2.3, P2.4 |
| A borrowed iPad or iPhone for half an hour | P2.6 |

## 8. Out of scope

The program on the parish PC with the screen on the projector (replacing
`deploy/windows/start-pilot.ps1`), a macOS build, Linux packaging (Flatpak or a distribution
package), automatic updates of the desktop program. Each needs its own decision first.
