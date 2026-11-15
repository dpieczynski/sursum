# Architecture

## Overview

```
 Android tablet (Chrome)  ─┐  Wi-Fi / LAN               Raspberry Pi 5 (next to the projector)
 Linux PC (Chromium)      ─┤
 Windows PC (Chrome/Edge) ─┼──────── router ── Ethernet (+PoE) ──┐
 (iPad, Firefox: no guarantee)                                    │
                                        ┌─────────────────────────▼─────────────────┐
                                        │ sursum-server (one binary, systemd)       │
                                        │  axum: HTTP API + WebSocket + embedded UI │
                                        │  StateActor: sole owner of state          │
                                        │  SQLite (WAL) on the /data partition      │
                                        └─────────▲─────────────────────────────────┘
                                                  │ ws://localhost (read-only)
                                        cage + Chromium --kiosk → HDMI-A-1 (forced 1080p60)
```

**All logic lives on the server**: parsing, slide splitting, order, navigation, program and
preview, search, editor validation and the screen layout (exclusion zones). Browsers draw and pick
font sizes (that needs font metrics). Nothing is implemented twice.

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
- The remote preview uses the same `@sursum/render` components, CSS, fonts and stage; all target
  clients use Blink, so the preview matches the screen.

## Exclusion zones (ADR 0008)

A per-device `ScreenProfile` lists rectangles of the 1920×1080 frame that are unusable (the dark
pillar in Tulce). `sursum-core::layout::content_region` picks the largest free rectangle inside the
safe area; the screen fits text into it and paints black masks over the zones.

## Persistence

SQLite in WAL mode with `synchronous=FULL`, `foreign_keys=ON`; daily and pre-import backups with
`VACUUM INTO`; `quick_check` at startup with automatic restore of the last good backup. The root
filesystem on the Pi is read-only; all writable data lives on `/data`.

See also: [protocol.md](protocol.md), [roadmap.md](roadmap.md), [decisions/](decisions/).
