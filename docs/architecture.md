# Architecture

## Overview

```text
 Android tablet (Chrome)  ─┐  Wi-Fi / LAN               Raspberry Pi 5 (next to the projector)
 Linux PC (Chromium)      ─┤
 Windows PC (Chrome/Edge) ─┼──────── router ── Ethernet (+PoE) ──┐
 (iPad, Firefox: no guarantee)                                    │
                                        ┌─────────────────────────▼─────────────────┐
                                        │ sursum-server (one binary, systemd)       │
                                        │  axum: HTTP API + WebSocket + embedded UI │
                                        │  StateActor: sole owner of state          │
                                        │  SQLite (WAL) in /var/lib/sursum          │
                                        │  (immutable A/B image, /persistent)       │
                                        └─────────▲─────────────────────────────────┘
                                                  │ ws://localhost (read-only)
                                        cage + Chromium --kiosk → HDMI-A-1 (forced 1080p60)
```

**All logic lives on the server**: parsing, slide splitting, order, navigation, program and
preview, search, editor validation and the screen layout (exclusion zones). Browsers draw and pick
font sizes (that needs font metrics). Nothing is implemented twice.

## Crates and apps (ADR 0014)

Everything is Rust. The apps are Leptos 0.8 CSR compiled to WebAssembly with Trunk.

| Crate | Role | Targets |
|---|---|---|
| `sursum-protocol` | shared serde types (data only) | native + wasm32 |
| `sursum-core` | domain logic | native (server) |
| `sursum-formats` | OpenSong / OpenLyrics | native |
| `sursum-server` | axum, WebSocket, SQLite, embedded apps | native |
| `sursum-slide` | shared slide rendering + font fitting | wasm32 (pure fit algorithm also native) |
| `sursum-client` | shared WebSocket client + state store | wasm32 |
| `apps/screen`, `apps/remote` | Leptos CSR apps | wasm32 (Trunk) |

Shared wasm crates share data and drawing, never domain logic.

## Server state

- One **StateActor** (a thread; rusqlite is synchronous) owns `AppState` and the only writing
  SQLite connection. Commands arrive over `mpsc` with a `oneshot` reply, so every change has one
  global order.
- Fan-out uses `tokio::sync::watch` with whole snapshots (`ScreenState`, `RemoteState`): slow
  clients skip intermediate states and never overflow.
- A global `rev: u32` increases on every client-visible change; entities have their own `revision`.

## Program vs. set (ADR 0005)

Going live compiles a `Deck` and stores a **frozen copy** in `Program`. Set edits and song edits
never touch it. Navigation inside the item uses the frozen deck; if the song changed meanwhile,
the deck is rebuilt on the next navigation and the position is found by `SlideKey`. If the live
item was removed, a *ghost position* remembers where it was, so next/previous still work.

## Restart policy (ADR 0003)

`runtime_state` stores the program together with the OS `boot_id`. Same `boot_id` (only the
process restarted) → full restore. Different `boot_id` (power cut, reboot) → restore the position
with the logo overlay; one tap shows the slide again. No dependency on the clock.

## Rendering

- Fixed 1920×1080 stage scaled with `transform: scale()`; safe-area margins per device.
- Two content layers (A/B) with **opaque** backgrounds: the next slide is drawn in the hidden
  layer, then faded in over the old one after `document.fonts.ready` and `img.decode()`.
  Interrupted transitions call `Animation.finish()`; only the newest state is drawn.
- Black/logo overlay above everything; masks for exclusion zones below the overlay.
- One font size per song (`fit_group`): binary search over integer px, **no line wrapping**
  first (lines come from the server), wrapping only if the minimum size does not fit.
- The remote preview uses the same `sursum-slide` Leptos components, CSS, fonts and stage; all
  target clients use Blink, so the preview matches the screen.

## Display engine portability (ADRs 0009, 0015)

Chromium under cage is the default. The screen follows an API contract that WPE WebKit (cog) also
supports, and the screen subset of the WebDriver tests runs blocking on WPE in CI, so switching to
`cog --platform=drm` stays a configuration change in the image (`kiosk.engine`), decided by
hardware measurements.

## Exclusion zones (ADR 0008)

A per-device `ScreenProfile` lists rectangles of the 1920×1080 frame that are unusable (the dark
pillar in Tulce). `sursum-core::layout::content_region` picks the largest free rectangle inside the
safe area; the screen fits text into it and paints black masks over the zones.

## Display power (ADR 0012)

The remote can turn the display on or put it to standby, because infrared remotes do not reach
the operator. A background task in `sursum-server::display_power` owns the display connection
(today PJLink over TCP 4352; CEC follows after the hardware spike), runs `DisplayPower` commands
sent straight from the WebSocket session, polls the state and reports only changes to the state
actor, so navigation never waits for a display. The PJLink codec is pure
(`sursum-core::display_power`) and checked against the specification's example; tests talk to a
fake projector over TCP. The backend is chosen in `config.toml` (`[display_power]`).

## Pictures and the logo

Uploaded pictures (posters, the parish logo) are stored once per content hash in
`<data>/assets/<sha256>.<ext>` (PNG, JPEG, WebP by magic bytes; atomic writes; 2 GB quota) and
served at `/assets/<id>` as immutable. Image slides and the logo are drawn inside the content
rectangle and decoded before they are shown.

## Rights (ADR 0016)

Anyone on the LAN may open the screen. Pairing with the PIN makes a remote an **operator**;
entering the PIN again unlocks **admin mode** on that device for 15 minutes (import, merge,
calibration, logo, devices). Rights are checked on every request and WebSocket command;
revoking a device closes its session. The server answers only to its own host names (DNS
rebinding) and file uploads need a custom header (no simple cross-site POSTs).

## Persistence

SQLite in WAL mode with `synchronous=FULL`, `foreign_keys=ON`; schema migrations are plain SQL
scripts applied in order and tracked with `PRAGMA user_version` (`sursum-server/src/db.rs`); daily and pre-import backups with
`VACUUM INTO`; `quick_check` at startup with automatic restore of the last good backup.

## Device image and updates (ADR 0013)

The Pi runs our own immutable image built with rpi-image-gen (`trixie-minbase-ab`, `image-rota`
layout): read-only A/B system slots, all writable state on `/persistent`. `/var/lib/sursum` is
slot-shared and holds the database, backups, assets, configuration and the application releases.
Application updates are signed `.sursum-app` bundles (ZIP: binary, manifest with its hash,
Ed25519 signature; `sursum-server bundle …`) installed into `releases/<version>/` and switched
atomically with the `current`/`previous` symlinks; the server reports readiness and liveness to
systemd (`Type=notify`, watchdog pinged only while the state actor answers). Restart supervision
with automatic rollback on the device and system updates (M7, A/B slots with a trial boot)
follow.

See also: [protocol.md](protocol.md), [roadmap.md](roadmap.md), [decisions/](decisions/).
