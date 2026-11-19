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

The same server also runs at home inside the **desktop program** `sursum` (ADR 0020): on
loopback, as a library without a screen, with the remote in a window of its own (WebView2 on
Windows, WebKitGTK 6.0 on Linux). Songs travel to the church as a `.sursum` file.

## Crates and apps (ADR 0014)

Everything is Rust. The apps are Leptos 0.8 CSR compiled to WebAssembly with Trunk.

| Crate | Role | Targets |
|---|---|---|
| `sursum-protocol` | shared serde types (data only) | native + wasm32 |
| `sursum-core` | domain logic | native (server) |
| `sursum-formats` | OpenSong import (songs, sets); OpenLyrics export is not built | native |
| `sursum-server` | axum, WebSocket, SQLite, embedded apps (binary and library) | native |
| `sursum-desktop` | the desktop program `sursum`: the server library plus a window, no logic | native (Windows, Linux) |
| `sursum-slide` | shared slide rendering + font fitting | wasm32 (pure fit algorithm also native) |
| `sursum-client` | shared WebSocket client (reconnect, heartbeat, status signal); the apps keep their own state | wasm32 |
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
- One text style for every screen (`sursum-slide/src/theme.rs`: centred white Lato Bold, 36 to
  120 px); it is drawing, not data, so it is not in the snapshot.
- One font size per song (`fit_group`): binary search over integer px, **no line wrapping**
  first (lines come from the server), wrapping only if the minimum size does not fit.
- The remote preview uses the same `sursum-slide` Leptos components, CSS, fonts and stage. The
  remotes no longer share one engine with the screen: Blink on Android, Windows and in the kiosk,
  WebKit in the Linux desktop window (ADRs 0020, 0021). A cross-engine test keeps
  the fitted font size within 1 px and the line count equal between WPE WebKit and Chromium.

## The remote (ADR 0017)

Three views as tabs: **Msza** (`apps/remote/src/live/`), **Zestaw** (`sets/`, `editor/`) and
**Ustawienia** (`settings/`), inside a shell (`shell.rs`) whose regions the stylesheets
(`apps/remote/style/`) arrange for phone, upright tablet and wide screens. The live area (what is
on screen, slides, controls) never moves; narrow screens show a live strip in the other views.
Everything shown is derived from `RemoteState` (`live/info.rs` only puts it into words); shared
parts (sheet, toast, banner, hold button, PIN field) are in `ui/`.

## Display engine portability (ADRs 0009, 0015)

Chromium under cage is the engine, tamed by managed policies, a short command line, a transparent
cursor theme, a localhost-only network for the kiosk unit and a session script that keeps the
display at 1080p60 (`deploy/kiosk/`). The screen follows an API contract that WPE WebKit also
supports, and the screen subset of the WebDriver tests runs blocking on WPE in CI. Whether WPE WebKit 2.54 on DRM (no compositor, our
rebuild of Debian's source package, WebKit's own MiniBrowser as the launcher) replaces Chromium
and cage is decided by a spike on the pilot Pi (ADR 0009 amendment of 2026-11-19); cog is legacy
and not used on devices.

## Exclusion zones (ADR 0008)

A per-device `ScreenProfile`, set only from the remote's calibration panel and stored in the
database, lists rectangles of the 1920×1080 frame that are unusable (the dark pillar in Tulce). `sursum-core::layout::content_region` picks the largest free rectangle inside the
safe area; the screen fits text into it and paints black masks over the zones.

## Display power (ADR 0012)

The remote can turn the display on or put it to standby, because infrared remotes do not reach
the operator. A background task in `sursum-server::display_power` owns the display connection
(HDMI-CEC through `/dev/cec0`, the image's default, or PJLink over TCP 4352), runs `DisplayPower` commands
sent straight from the WebSocket session, polls the state and reports only changes to the state
actor, so navigation never waits for a display. The PJLink codec is pure
(`sursum-core::display_power`) and checked against the specification's example; tests talk to a
fake projector over TCP. The CEC backend (`display_power/cec.rs`, Linux only) was checked on a TV
with the pilot Pi; both churches' displays are still to be checked on site. The backend is chosen
in `config.toml` (`[display_power]`).

## Pictures and the logo

Uploaded pictures (posters, the parish logo) are stored once per content hash in
`<data>/assets/<sha256>.<ext>` (PNG, JPEG, WebP by magic bytes; atomic writes; 2 GB quota) and
served at `/assets/<id>` as immutable. Image slides and the logo are drawn inside the content
rectangle and decoded before they are shown.

## Rights (ADRs 0016, 0024)

Anyone on the LAN may open the screen. Pairing with the PIN lets a remote do everything; there is
no second level. Pairing is checked on every request and WebSocket command; revoking a device
closes its session. The server answers only to its own host names (DNS
rebinding) and file uploads need a custom header (no simple cross-site POSTs).

## Persistence

SQLite in WAL mode with `synchronous=FULL`, `foreign_keys=ON`; schema migrations are plain SQL
scripts applied in order and tracked with `PRAGMA user_version` (`sursum-server/src/db.rs`); backups at every start and before imports, merges, updates and restores with
`VACUUM INTO` (14 kept); `quick_check` at startup with automatic restore of the last good backup.

## Device image and updates (ADR 0013)

The Pi runs our own immutable image built with rpi-image-gen (`trixie-minbase-ab`, `image-rota`
layout): read-only A/B system slots, all writable state on `/persistent`. `/var/lib/sursum` is
slot-shared and holds the database, backups, assets, configuration and the application releases.
Application updates are signed `.sursum-app` bundles (ZIP: binary, manifest with its hash,
Ed25519 signature; `sursum-server bundle …`) installed into `releases/<version>/` and switched
atomically with the `current`/`previous` symlinks; the server reports readiness and liveness to
systemd (`Type=notify`, watchdog pinged only while the state actor answers). A new release runs
on trial: the previous release counts the starts (`bundle guard`) and switches back, with the
matching database copy, if the new one does not run healthily for 30 s within three starts.

System updates (ADR 0019) are a second, separate path for everything outside the server binary
(kiosk unit, browser, policies, kernel): a signed `.sursum-system` file with the raw boot and
system images is uploaded from the remote, written to the other A/B slot by a root unit running
the read-only system's own copy of the server, and started once through the firmware's tryboot;
it is kept only if the server and the kiosk come up healthy. Both paths were run on the pilot Pi 5,
including a deliberate rollback.

Two update paths are one too many: ADR 0022 (2026-11-19, not built yet) moves the server into
the read-only system and keeps only the system update.

See also: [protocol.md](protocol.md), [roadmap.md](roadmap.md), [decisions/](decisions/).
