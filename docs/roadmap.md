# Roadmap

Update the checkboxes as work lands. Acceptance criteria are the definition of done for each
milestone. ⚠ = assumption to verify in a spike before relying on it.

## M0 — Skeleton

- [x] Git repository, `.gitignore`, `.gitattributes`, `.editorconfig`
- [x] License (GPL-3.0-or-later), README, CONTRIBUTING (DCO), `fixtures/LICENSES.md`
- [x] Guidance files (`CLAUDE.md` ×4), docs, ADRs
- [ ] Devcontainer (Debian trixie: rustup + `wasm32-unknown-unknown`, trunk 0.22, wasm-bindgen-cli matching Cargo.lock, binaryen, leptosfmt, zig + cargo-zigbuild, chromium + chromium-driver, cog + wpewebkit-webdriver; no Node/Python)
- [ ] Cargo workspace: `sursum-protocol`, `sursum-core`, `sursum-formats`, `sursum-server`, `sursum-slide`, `sursum-client`, `apps/screen`, `apps/remote`, `e2e`; `default-members` = native crates; `rust-toolchain.toml`, `deny.toml`
- [ ] Leptos 0.8 CSR skeletons for both apps with Trunk (`index.html`, `Trunk.toml` with `/api` + `/ws` proxy ⚠ WS proxy)
- [ ] rust-embed serving both `apps/*/dist` ⚠ debug builds read from disk, ⚠ `allow_missing` when dist is absent
- [ ] Bundled OFL fonts in `assets/fonts/` with `LICENSES.md`
- [ ] `justfile`, SPDX check script, third-party license page (`cargo about` + fonts) at `/licenses`
- [ ] First WebDriver test (thirtyfour + chromium-driver ⚠ in the container): screen opens with a black background
- [ ] Same test on WPE WebKit: `wpewebkit-webdriver` + `cog --automation --platform=headless` ⚠ capability format (ADR 0015)
- [ ] GitHub Actions CI (fmt, leptosfmt, clippy native + wasm32, nextest, wasm-bindgen-test, trunk build + WASM size check, WebDriver e2e (Chromium; screen subset also on WPE, blocking), cargo-deny, cross builds)

**Done when:** `just dev` gives auto-reload for both apps with a working WS echo using
`sursum-protocol` types; `just build` produces one binary serving both apps; the first WebDriver
test passes in Chromium and WPE; CI is green.

## M1 — Screen, remote and one song on a laptop

- [ ] `sursum-protocol`: messages and snapshots with `insta` JSON snapshots
- [ ] core: song model, slide compiler, program state machine (next/prev/goto/overlay), PD seed song
- [ ] core: `content_region` (exclusion zones) and `ScreenLayout` in the snapshot; profile from `config.toml`
- [ ] server: state actor, `watch` fan-out, `/ws/screen`, `/ws/remote`, `runtime_state` with `boot_id`
- [ ] `sursum-slide`: `Stage`, `LayeredStage` (A/B + overlay + masks), `fit_deck` (pure algorithm + DOM measuring), local fonts
- [ ] `sursum-client`: WebSocket with reconnect, heartbeat, stale-`rev` dropping; signal store
- [ ] remote: current/next preview, next/prev, section chips, BLACK/LOGO
- [ ] On-site display power spike: `cec-ctl` on both displays (on / standby / power status), projector LAN (PJLink) and RS-232 check in Tulce; record result in ADR 0012
- [ ] Hardware spike on a Pi 5: cage + Chromium (flags ⚠, cursor ⚠, white flash ⚠), crossfade smoothness; WPE/cog comparison (`--platform=drm`, `--bg-color=black`, `--webprocess-failure=exit`); first look at the Tulce pillar
- [ ] Screen WebDriver subset blocking on WPE in CI; cross-engine fit test (WPE screen vs Chromium remote)

**Done when:** WebDriver tests pass (black first frame; last slide stays on disconnect; reconnect
without a transition; one font size per song; preview matches screen font size and line count;
text never enters an exclusion zone); manual run on a Linux laptop + Android tablet.

## M2 — OpenSong import, library, search, editor

- [ ] `sursum-formats`: OpenSong songs (CLI + ZIP upload), import report, idempotent re-import
- [ ] Verify extended OpenSong conventions ⚠ (see [opensong-format.md](opensong-format.md))
- [ ] SQLite schema V1, migrations, backups, `quick_check` + auto-restore
- [ ] In-memory search with Polish folding (ł!) and ranking
- [ ] Keyboard-friendly song editor with server-compiled preview; revision history

**Done when:** every convention has a synthetic fixture test; the real parish library imports
locally without critical errors; search by number/title/fragment works regardless of diacritics;
p95 < 30 ms for 5000 synthetic songs (laptop) and measured on the Pi; a new song can be entered
without a mouse.

## M3 — Sets, live editing, pairing, shared library

- [ ] Sets CRUD/activate, `SetOp`, frozen program with ghost position, "Show now", Text/Image/Blank items
- [ ] OpenSong set import
- [ ] Multi-remote `Stale` rule, PIN pairing, roles
- [ ] `.sursum` export/import with merge and dry-run
- [ ] Screen calibration panel (grid, drag exclusion zone on the preview)

**Done when:** WebDriver tests prove set edits never change the screen; simultaneous "next" from two
remotes advances once; merging two libraries with a planted conflict yields a correct report and
no data loss; the pillar zone can be set from the remote in < 2 minutes.

## P1 — Windows pilot

- [ ] `start-pilot.ps1` (server + kiosk on the second monitor ⚠), operator guide

**Done when:** one real service in one church with OpenSong ready as plan B; feedback filed.

## M4 — Raspberry Pi deployment

- [ ] Device image with rpi-image-gen (ADR 0013): `trixie-minbase-ab` + `image-rota`, layers `sursum-kiosk` and `sursum-device`, slot-shared `/var/lib/sursum`, pinned tool version
- [ ] In the image: systemd units, silent boot + splash, forced HDMI + fallback EDID ⚠, nftables, Avahi, kiosk watchdog, hardware/systemd watchdogs
- [ ] Image build in CI on arm64 ⚠ (runner availability, privileged container); artifacts `.img` and `.update.tar.zst`
- [ ] Signed application bundles (`.sursum-app`, Ed25519): browser upload, USB stick, `just deploy`; DB snapshot before migration; automatic rollback
- [ ] `kiosk.engine = chromium | cog` in the `sursum-kiosk` layer; weekly CI build of the cog image variant
- [ ] PoE evaluation
- [ ] Display power (ADR 0012): `CecDisplay`, `PjLinkDisplay`, state indicator and buttons in the remote, udev rule for `/dev/cec0` (PJLink may land earlier, it does not need the Pi)

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

- [ ] Default mapping, ignore key repeat, learn mode, per-device mapping
- [ ] Android: on-screen keyboard with a hardware keyboard paired ⚠ (docs)

**Done when:** mapping tests pass; the organist runs a full service with a pedal only.

## Later

M6 optional HTTPS (device CA) + PWA + wake lock · M7 offline A/B **system** updates (rpi-image-gen
bundle or RAUC, trial boot, rollback) and an x86_64 mini-PC image (`mkosi` + `systemd-sysupdate`) ·
M8 liturgical calendar/readings (licensing!) · M9 video backgrounds (HEVC on Pi 5) · M10 multiple
screens · M11 internet sync · iPad (WebKit) support. **MIDI: dropped** (ADR 0002).
