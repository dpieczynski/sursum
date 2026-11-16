# WebSocket protocol

Draft for M0/M1. The types live in the shared crate `crates/sursum-protocol`, compiled by the
server (native) and by both Leptos apps (wasm32). There is no code generation: both sides use the
same Rust types, so they cannot drift apart within one build.

## Endpoints

- `GET /ws/screen`: read-only, no authentication, loopback and LAN only.
- `GET /ws/remote`: requires the device cookie set during PIN pairing; `Origin` and `Host` are
  validated on upgrade (protection against cross-site WebSocket hijacking and DNS rebinding).
- REST under `/api/…` for large or on-demand data: search, song drafts and preview, import,
  library export/merge, pairing, admin mode and the device list.

### Rights (ADR 0016)

| Who | May |
|---|---|
| anyone on the LAN | `/screen/`, `/ws/screen`, `/api/session`, `/api/pair`, `/healthz`, `/licenses` |
| paired remote (operator) | `/ws/remote` commands except setup ones, search, editor, `/api/library/export`, `POST/DELETE /api/admin` |
| admin mode (PIN again, 15 min) | `/api/import/opensong`, `/api/library/merge`, `/api/devices`, `/api/devices/{id}/revoke`, commands `SetScreenProfile` and `Calibrate` |

Without a PIN everyone has admin rights. Refusals: HTTP 401 (not paired) or 403 (needs admin
mode); WebSocket commands answer `Rejected { code: forbidden }`.

JSON text frames (`serde_json` on both sides, readable in browser dev tools); serde enums tagged
with `"t"`; default Rust (snake_case) field names, since no JavaScript consumes them.

## Client → server

```text
Hello   { protocol, role: "screen"|"remote", clientId, buildId, clientTime? }
Cmd     { seq, baseRev, cmd: Command }
Ping    { }

Command = Next | Prev | NextItem | PrevItem                // relative: subject to `Stale`
        | GoTo { item_id, slide_index } | GoToItemNumber { number } | GoToSlide { slide_index }
        | SetOverlay { overlay: "none"|"black"|"logo" } | ToggleBlack
        | ShowNow { song_id }           // insert after the live item and go live
        | AddSong { song_id } | AddText { title, text }
        | RemoveItem { item_id } | MoveItem { item_id, to_index } | UndoRemove
        | NewSet { name } | ActivateSet { set_id }
        | SetScreenProfile { profile } | Calibrate { on }   // admin mode, ADR 0008
        // planned: DisplayPower { action: "on"|"standby" }  (HDMI-CEC / PJLink, ADR 0012)
```

## Server → client

```text
Welcome   { protocol, buildId, device?, serverTime }
Screen    { rev, live?: { deck, slideIndex }, upcoming?: Deck, overlay, theme, transitionMs, logo?, layout: ScreenLayout }
Remote    { rev, program: ProgramView, preview?: SlideRef, activeSet?: SetView, presence: [DeviceView],
            displayPower: "unknown"|"on"|"standby"|"warmingUp"|"coolingDown"|{ unavailable: reason } }
CmdResult { seq, outcome: Applied{rev} | Stale{rev} | Rejected{code} | Conflict{current} }
Notice    { code, params }              // i18n codes, never ready-made text
Pong      { }
```

## Snapshots, not patches

Each domain is small (a deck is a few KB, a set < 10 KB), so the server sends whole snapshots.
There is no event log and no patch ordering. Within one connection messages arrive in order, and
after a reconnect the snapshot that follows `Welcome` replaces the client state; server revisions
restart after a server restart, so clients apply every snapshot instead of comparing `rev`.

## Conflict rules

- **Relative navigation** (`Next`, `Prev`, `NextItem`, `PrevItem`) carries `baseRev`. Applied if
  `baseRev == program.updatedRev` or the last program change came from the same client (fast
  repeated presses work); otherwise `Stale`. Two operators pressing "next" at once advance once.
- **Absolute commands** (`GoTo…`, `SetOverlay`, `ShowNow`) always apply; last writer wins.
- **Set operations** address items by stable IDs; a missing ID returns `Rejected(NotFound)`.
- **Song edits** use optimistic locking on `revision`; a mismatch returns `Conflict`.
- **Display power** commands are executed asynchronously; failures return
  `Rejected(DisplayUnreachable | DisplayBusy | DisplayAuthFailed)` and never block navigation.

## Reconnect

`Hello` → `Welcome` + full snapshot. The screen compares `deck.deckId` and the slide key with what
it shows; if equal, it redraws nothing (no transition). Backoff 250 ms → 5 s with jitter; ping
every 5 s; no pong for 15 s → reconnect. A different `protocol` or `buildId` makes the page reload
(the screen waits until it shows an overlay or the program changes).

## Type notes

- In `sursum-protocol`: messages, commands, snapshots, `Deck`/`Slide`, `Theme`, `Overlay`,
  `ScreenLayout`, `ScreenProfile`, `Rect`, `DisplayPowerState`, REST DTOs, error and notice codes.
  Not in it (server only): DB rows, parser AST, the internal `Program`, any logic.
- The crate depends only on `serde` (plus e.g. `uuid` with the `serde` feature) so it compiles for
  `wasm32-unknown-unknown` without tokio or `getrandom`; CI builds it for both targets.
- `insta` JSON snapshots of every message guard the wire format: after an update, a screen may
  still run old code until it reloads, so field renames must be deliberate.
- Field names in the pseudo-code above are illustrative; the Rust structs are authoritative.
