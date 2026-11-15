# WebSocket protocol

Draft for M0/M1. Rust types in `sursum-core::protocol` are the source of truth; TypeScript is
generated with ts-rs into `frontend/packages/protocol/src/generated/`.

## Endpoints

- `GET /ws/screen`: read-only, no authentication, loopback and LAN only.
- `GET /ws/remote`: requires the device cookie set during PIN pairing; `Origin` and `Host` are
  validated on upgrade (protection against cross-site WebSocket hijacking and DNS rebinding).
- REST under `/api/…` for large or on-demand data: search, song details, import/export, assets,
  pairing, `POST /api/songs/compile` (editor preview).

JSON text frames; serde enums tagged with `"t"`, camelCase fields.

## Client → server

```
Hello   { protocol, role: "screen"|"remote", clientId, buildId, clientTime? }
Cmd     { seq, baseRev, cmd: Command }
Ping    { }

Command = Next | Prev | NextItem | PrevItem
        | GoTo { itemId, slideKey } | GoToItemNumber { n } | GoToSection { label }
        | SetOverlay { overlay: "none"|"black"|"logo" } | ToggleBlack
        | ShowNow { songId }            // insert after the live item and go live
        | ActivateSet { setId }
        | SetOp { setId, op: Add{after?, item} | Remove{itemId} | Move{itemId, before?} | UpdateItem{itemId, item} }
```

## Server → client

```
Welcome   { protocol, buildId, device?, serverTime }
Screen    { rev, live?: { deck, slideIndex }, upcoming?: Deck, overlay, theme, transitionMs, logo?, layout: ScreenLayout }
Remote    { rev, program: ProgramView, preview?: SlideRef, activeSet?: SetView, presence: [DeviceView] }
CmdResult { seq, outcome: Applied{rev} | Stale{rev} | Rejected{code} | Conflict{current} }
Notice    { code, params }              // i18n codes, never ready-made text
Pong      { }
```

## Snapshots, not patches

Each domain is small (a deck is a few KB, a set < 10 KB), so the server sends whole snapshots.
There is no event log and no patch ordering. Clients drop messages with a `rev` lower than the
last one seen.

## Conflict rules

- **Relative navigation** (`Next`, `Prev`, `NextItem`, `PrevItem`) carries `baseRev`. Applied if
  `baseRev == program.updatedRev` or the last program change came from the same client (fast
  repeated presses work); otherwise `Stale`. Two operators pressing "next" at once advance once.
- **Absolute commands** (`GoTo…`, `SetOverlay`, `ShowNow`) always apply; last writer wins.
- **Set operations** address items by stable IDs; a missing ID returns `Rejected(NotFound)`.
- **Song edits** use optimistic locking on `revision`; a mismatch returns `Conflict`.

## Reconnect

`Hello` → `Welcome` + full snapshot. The screen compares `deck.deckId` and the slide key with what
it shows; if equal, it redraws nothing (no transition). Backoff 250 ms → 5 s with jitter; ping
every 5 s; no pong for 15 s → reconnect. A different `protocol` or `buildId` makes the page reload
(the screen waits until it shows an overlay or the program changes).

## Type notes

- Counters are `u32`. Any `u64` needs `#[ts(type = "number")]` ⚠ (verify ts-rs 12 mapping in M0).
- Generated: messages, commands, snapshots, `Deck`/`Slide`, `Theme`, `Overlay`, `ScreenLayout`,
  `ScreenProfile`, `Rect`, REST DTOs, error and notice codes. Not generated: DB rows, parser AST,
  the internal `Program`.
