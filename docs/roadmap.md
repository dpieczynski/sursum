# Roadmap

Update the checkboxes as work lands. Acceptance criteria are the definition of done for each
milestone. ⚠ = assumption to verify in a spike before relying on it.

## M0 — Skeleton

- [x] Git repository, `.gitignore`, `.gitattributes`, `.editorconfig`
- [x] License (GPL-3.0-or-later), README, CONTRIBUTING (DCO), `fixtures/LICENSES.md`
- [x] Guidance files (`CLAUDE.md` ×4), docs, ADRs
- [x] Devcontainer definition (Debian trixie: rustup + `wasm32-unknown-unknown`, trunk 0.22, wasm-bindgen-cli 0.2.130, binaryen, leptosfmt, chromium + chromium-driver, cog + wpewebkit-webdriver; no Node/Python). Not yet built/tried in VS Code; zig + cargo-zigbuild still to add
- [x] Browsers image for e2e (`e2e/browsers.Containerfile`, podman, host network): Chromium 156 + chromedriver, cog 0.18.4 + WPEWebDriver (WPE WebKit 2.48.3) + Mesa
- [x] Cargo workspace: `sursum-protocol`, `sursum-core`, `sursum-formats`, `sursum-server`, `sursum-slide`, `sursum-client`, `apps/screen`, `apps/remote`, `e2e`; `default-members` = native crates; `rust-toolchain.toml`, `deny.toml`
- [x] Leptos 0.8 CSR apps with Trunk (`index.html`, `Trunk.toml` with `/ws` proxy; WebSocket upgrade through `trunk serve` verified)
- [x] rust-embed serving both `apps/*/dist`; `allow_missing` verified (server compiles without a frontend build; `/screen/` then serves a black page)
- [x] Bundled OFL fonts in `assets/fonts/` with `LICENSES.md` (Lato 400/700, latin + latin-ext)
- [x] `justfile`, SPDX check script, `deny.toml` (licenses, bans, sources, advisories pass)
- [x] Third-party license page (`cargo about` + fonts) at `/licenses`, linked from the remote
- [x] First WebDriver tests (thirtyfour 0.38 + chromedriver in the container)
- [x] Same tests on WPE WebKit: capability `wpe:browserOptions { binary: "cog", args: ["--automation", "--platform=headless", "--bg-color=black"] }` verified; WPE needs Mesa (`libgles2`, `libegl1`, `libgl1-mesa-dri`) or its renderer crashes
- [x] Cross-compilation verified locally with zig 0.17.1 + cargo-zigbuild 0.24.1: `aarch64-unknown-linux-gnu.2.36` and `x86_64-pc-windows-gnu` (both 11 MB with the apps embedded)
- [x] GitHub Actions CI written, not yet run on GitHub (`just test-wasm` added 2026-11-16) (fmt, leptosfmt, clippy native + wasm32, nextest, wasm-bindgen-test, trunk build + WASM size check, WebDriver e2e (Chromium; screen subset also on WPE, blocking), cargo-deny, cross builds)

**Done when:** `just dev` gives auto-reload for both apps with a working WS echo using
`sursum-protocol` types; `just build` produces one binary serving both apps; the first WebDriver
test passes in Chromium and WPE; CI is green.

## M1 — Screen, remote and one song on a laptop

- [x] `sursum-protocol`: messages and snapshots with `insta` JSON snapshots
- [x] core: song model, slide compiler, program state machine (next/prev/goto/overlay, Stale rule, frozen deck), PD + synthetic demo songs
- [x] core: `content_region` (exclusion zones) and `ScreenLayout` in the snapshot; profile from `config.toml`
- [x] server: state actor, `watch` fan-out, `/ws/screen`, `/ws/remote`, Origin check, local-network guard, `runtime_state` with `boot_id` (migrations hand-rolled with `PRAGMA user_version`: `rusqlite_migration` 2.7 requires rusqlite 0.41)
- [x] `sursum-slide`: `Stage`, `LayeredStage` (A/B + overlay + masks), `fit_deck` (pure algorithm + DOM measuring), local fonts
- [x] `sursum-client`: WebSocket with reconnect (backoff + jitter), heartbeat, Hello/Welcome, build-id reload. No stale-`rev` dropping: messages arrive in order per connection and server revisions restart after a server restart, so clients apply every snapshot
- [x] remote: current/next preview, next/prev, item prev/next, section chips, set list, BLACK/LOGO, pedal keys (PgDn/PgUp/arrows/space/B). UI texts via a compile-time message enum (`apps/remote/src/messages.rs`); `leptos_i18n` deferred until a second language is needed
- [ ] On-site display power spike: `cec-ctl` on both displays (on / standby / power status), projector LAN (PJLink) and RS-232 check in Tulce; record result in ADR 0012
- [ ] Hardware spike on a Pi 5: cage + Chromium (flags ⚠, cursor ⚠, white flash ⚠), crossfade smoothness; WPE/cog comparison (`--platform=drm`, `--bg-color=black`, `--webprocess-failure=exit`); first look at the Tulce pillar
- [x] Screen WebDriver tests pass on WPE locally; cross-engine fit test (WPE screen vs Chromium remote) passes. Blocking in CI once CI runs

**Done when:** WebDriver tests pass (black first frame; last slide stays on disconnect; reconnect
without a transition; one font size per song; preview matches screen font size and line count;
text never enters an exclusion zone); manual run on a Linux laptop + Android tablet.

Status 2026-11-16: all listed WebDriver tests pass on Chromium and (screen) WPE WebKit; the
manual run on a laptop + tablet and both hardware spikes are still open.

## M2 — OpenSong import, library, search, editor

- [x] `sursum-formats`: OpenSong songs, import report, idempotent re-import; CLI `sursum-server import-opensong <Songs dir>` (875 real songs, 0 failures)
- [x] ZIP upload of an OpenSong folder from the browser (`POST /api/import/opensong`, backup first, library reloaded, one-line report in the remote)
- [x] Extended OpenSong conventions checked against a real Polish library (see [opensong-format.md](opensong-format.md)); numbered lines still ⚠
- [x] SQLite schema (migration 2: `songs` with revisions, `song_history`)
- [x] Backups (`VACUUM INTO`, 14 kept, at startup and before imports), `quick_check` at startup + automatic restore of the newest backup (damaged file kept in `corrupt/`)
- [x] In-memory search with Polish folding (ł!) and ranking; ~1.2 ms per query over 5000 songs (laptop); remote search panel with Show now / Add / Edit
- [x] Keyboard-friendly song editor (OpenSong markup, `---` optional break) with server-compiled preview drawn by `sursum-slide`; revision history and optimistic locking (conflict detection)

**Done when:** every convention has a synthetic fixture test; the real parish library imports
locally without critical errors; search by number/title/fragment works regardless of diacritics;
p95 < 30 ms for 5000 synthetic songs (laptop) and measured on the Pi; a new song can be entered
without a mouse.

## M3 — Sets, live editing, pairing, shared library

- [x] Persistent sets (create, activate), live editing (move, hold-to-remove, undo), frozen program with ghost position, "Show now", announcements (text items), the parish logo for the LOGO button (admin upload, drawn inside the content area, pre-decoded), pictures (content-addressed uploads in `<data>/assets/`, PNG/JPEG/WebP, decoded before the fade) and pauses (blank items); assets are not yet part of backups or `.sursum` files
- [x] Remote: two-column landscape layout (previews left, library/set right, controls bottom)
- [x] Remote: "#" keypad to jump to item N of the set (plan §7), no system keyboard needed
- [x] OpenSong set import (folder and ZIP): songs by path/file name/title, `presentation` override, custom and scripture groups as text items, image groups reported; re-import replaces the set
- [x] Multi-remote `Stale` rule (M1)
- [x] PIN pairing: `sursum-server set-pin`, salted hash, rate-limited `/api/pair` with doubling lockout, `HttpOnly; SameSite=Strict` device cookie (only its hash stored), protected `/ws/remote` and `/api/*`, pairing screen in the remote; screen stays open; a device without a PIN is closed and its first remote sets the PIN (`/api/setup`, ADR 0016 amendment), `--open-without-pin` for development only
- [x] Roles: paired remote = operator; admin mode by entering the PIN again (15 min, memory only) for import, merge, calibration and the device list; revocation closes the remote's session at once (ADR 0016)
- [x] Deleting songs (admin, hold in the editor): soft delete with history kept; tombstones travel in `.sursum` files and delete only the same or an older version on the other device; histories from merged files are remembered, so stale files never resurrect or duplicate songs; merge and import run as one transaction
- [x] `.sursum` library file: export (`GET /api/library/export`, ZIP with `manifest.json` + `songs.jsonl` incl. revision history), merge with mandatory dry run in the remote; fast-forward by ancestor hashes, newer-local kept, conflicts stored as a copy, backup before merging (sets and soft deletions not yet carried)
- [x] Screen calibration panel: hold to open, test card on the screen and the tablet, drag a zone or type exact numbers, safe area margins; stored in settings, validated on the server (ADR 0008 amendment)

**Done when:** WebDriver tests prove set edits never change the screen; simultaneous "next" from two
remotes advances once; merging two libraries with a planted conflict yields a correct report and
no data loss; the pillar zone can be set from the remote in < 2 minutes.

## P1 — Windows pilot

- [x] `deploy/windows/start-pilot.ps1` written (server + Edge kiosk on the non-primary monitor + remote) — ⚠ not yet run on Windows
- [x] Operator guide in Polish: `docs/operator-guide-pl.md`
- [ ] Try the pilot script on the parish PC; Windows has no boot id yet, so every start shows the logo

**Done when:** one real service in one church with OpenSong ready as plan B; feedback filed.

## M4 — Raspberry Pi deployment

- [ ] Device image with rpi-image-gen (ADR 0013): `trixie-minbase-ab` + `image-rota`, layers `sursum-kiosk` and `sursum-device`, slot-shared `/var/lib/sursum`, pinned tool version
- [x] Server side of the screen guard (plan §11.3): `[screen_guard] restart_command = [...]`, `grace_s` (default 15 s; 60 s after startup) in `config.toml`; only a screen on this machine (loopback: the kiosk) counts; remotes warn while none is connected. On the device: `systemctl restart sursum-kiosk` plus a polkit rule ⚠
- [x] Server side of the systemd watchdog: `Type=notify` readiness and `WATCHDOG=1` pings only while the state actor answers (checked under systemd on a desktop); draft `deploy/systemd/sursum.service` ⚠
- [ ] In the image: systemd units, silent boot + splash, forced HDMI + fallback EDID ⚠, nftables, Avahi, kiosk watchdog, hardware/systemd watchdogs
- [ ] Image build in CI on arm64 ⚠ (runner availability, privileged container); artifacts `.img` and `.update.tar.zst`
- [x] Signed application bundles, core (ADR 0013 amendment): `.sursum-app` ZIP with Ed25519-signed manifest, `bundle keygen/create/install/rollback`, DB backup before switching, atomic `current`/`previous` symlinks, `just app-bundle`
- [ ] Bundles on the device: systemd units on `current/`, restart + `/healthz` with automatic rollback, browser upload, USB stick, `just deploy`
- [ ] `kiosk.engine = chromium | cog` in the `sursum-kiosk` layer; weekly CI build of the cog image variant
- [ ] PoE evaluation
- [x] Display power, PJLink part (ADR 0012 amendment): codec checked against the JBMIA 1.04 example, background task with polling, state indicator, "on" tap and 1.5 s standby hold in the remote, tested against a fake projector (integration + e2e)
- [ ] Display power, CEC part: `cec_linux` backend, udev rule for `/dev/cec0` (needs the hardware spike)

**Done when:** a device flashed from the CI image needs no manual steps beyond configuration (PIN,
address, screen profile) and `apt install` on it fails (read-only root); app updates via browser
and USB work, a deliberately broken bundle rolls back by itself with the matching database, and a
bundle with a bad signature is rejected; power-on shows only black/logo then content (HDMI
capture); 100 power cuts without DB damage, logo + position after each boot; late projector power-on and HDMI replug pass; killed
Chromium/server recovers in < 5 s; Tulce pillar zone calibrated on site and text readable from the
last pew; in both churches the display turns on and to standby from the operator position and the
reported state matches reality in 20 consecutive tries (or a documented "unavailable" reason with
a hardware plan).

## M5 — Pedal and keyboard shortcuts

- [x] Default mapping, ignore key repeat, learn mode, per-device mapping (kept in the browser's `localStorage` — it belongs to the tablet the pedal is paired with and works without pairing; the plan's `devices.prefs_json` is not needed so far); in text fields only PageUp/PageDown act
- [ ] Android: on-screen keyboard with a hardware keyboard paired ⚠ (docs)

**Done when:** mapping tests pass; the organist runs a full service with a pedal only.

## Later

M6 optional HTTPS (device CA) + PWA + wake lock · M7 offline A/B **system** updates (rpi-image-gen
bundle or RAUC, trial boot, rollback) and an x86_64 mini-PC image (`mkosi` + `systemd-sysupdate`) ·
M8 liturgical calendar/readings (licensing!) · M9 video backgrounds (HEVC on Pi 5) · M10 multiple
screens · M11 internet sync · iPad (WebKit) support. **MIDI: dropped** (ADR 0002).
