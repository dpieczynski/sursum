# Roadmap

Update the checkboxes as work lands. Acceptance criteria are the definition of done for each
milestone. ⚠ = assumption to verify in a spike before relying on it.

## M0 — Skeleton

- [x] Git repository, `.gitignore`, `.gitattributes`, `.editorconfig`
- [x] License (GPL-3.0-or-later), README, CONTRIBUTING (DCO), `fixtures/LICENSES.md`
- [x] Guidance files (`CLAUDE.md` ×4), docs, ADRs 0001–0011
- [ ] Devcontainer (Debian trixie: rustup, Node 26, pnpm 12, zig + cargo-zigbuild, Playwright deps ⚠)
- [ ] Cargo workspace (`sursum-core`, `sursum-formats`, `sursum-server`), `rust-toolchain.toml`, `deny.toml`
- [ ] pnpm workspace (`protocol`, `render`, `i18n`, `screen`, `remote`, `e2e`)
- [ ] ts-rs pipeline → `@sursum/protocol` (`just bindings`) and drift check ⚠ u64 mapping
- [ ] rust-embed serving both apps ⚠ debug builds read from disk
- [ ] `justfile`, SPDX check script, third-party license page (`cargo about` + pnpm) at `/licenses`
- [ ] GitHub Actions CI (fmt, clippy, nextest, bindings drift, lint, svelte-check, vitest, Playwright, cargo-deny, cross builds)

**Done when:** `just dev` gives HMR for both apps with a working WS echo; `just build` produces one
binary serving both frontends; CI is green.

## M1 — Screen, remote and one song on a laptop

- [ ] core: song model, slide compiler, program state machine (next/prev/goto/overlay), PD seed song
- [ ] core: `content_region` (exclusion zones) and `ScreenLayout` in the snapshot; profile from `config.toml`
- [ ] server: state actor, `watch` fan-out, `/ws/screen`, `/ws/remote`, `runtime_state` with `boot_id`
- [ ] render: `Stage`, `LayeredStage` (A/B + overlay + masks), `fitDeck`, local fonts
- [ ] remote: current/next preview, next/prev, section chips, BLACK/LOGO
- [ ] Hardware spike on a Pi 5: cage + Chromium (flags ⚠, cursor ⚠, white flash ⚠), crossfade smoothness; WPE/cog comparison; first look at the Tulce pillar

**Done when:** Playwright tests pass (black first frame; last slide stays on disconnect; reconnect
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

**Done when:** Playwright proves set edits never change the screen; simultaneous "next" from two
remotes advances once; merging two libraries with a planted conflict yields a correct report and
no data loss; the pillar zone can be set from the remote in < 2 minutes.

## P1 — Windows pilot

- [ ] `start-pilot.ps1` (server + kiosk on the second monitor ⚠), operator guide

**Done when:** one real service in one church with OpenSong ready as plan B; feedback filed.

## M4 — Raspberry Pi deployment

- [ ] `setup-pi.sh`, systemd units, silent boot, forced HDMI + fallback EDID ⚠, nftables, Avahi
- [ ] Read-only root ⚠ + `/data` partition, kiosk watchdog, hardware/systemd watchdogs
- [ ] `just deploy` with release dirs and rollback; PoE evaluation

**Done when:** power-on shows only black/logo then content (HDMI capture); 100 power cuts without
DB damage, logo + position after each boot; late projector power-on and HDMI replug pass; killed
Chromium/server recovers in < 5 s; Tulce pillar zone calibrated on site and text readable from the
last pew.

## M5 — Pedal and keyboard shortcuts

- [ ] Default mapping, ignore key repeat, learn mode, per-device mapping
- [ ] Android: on-screen keyboard with a hardware keyboard paired ⚠ (docs)

**Done when:** mapping tests pass; the organist runs a full service with a pedal only.

## Later

M6 optional HTTPS (device CA) + PWA + wake lock · M7 system image (rpi-image-gen), web update ·
M8 liturgical calendar/readings (licensing!) · M9 video backgrounds (HEVC on Pi 5) · M10 multiple
screens · M11 internet sync · iPad (WebKit) support. **MIDI: dropped** (ADR 0002).
