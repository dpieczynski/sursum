# WebSocket protocol

Draft for M0/M1. The types live in the shared crate `crates/sursum-protocol`, compiled by the
server (native) and by both Leptos apps (wasm32). There is no code generation: both sides use the
same Rust types, so they cannot drift apart within one build.

## Endpoints

- `GET /ws/screen`: read-only, no authentication, loopback and LAN only.
- `GET /ws/remote`: requires the device cookie set during PIN pairing; `Origin` and `Host` are
  validated on upgrade (protection against cross-site WebSocket hijacking and DNS rebinding).
- REST under `/api/…` for large or on-demand data: search, song drafts and preview, import,
  library export/merge, pairing, admin mode, the device list and application updates.

### Rights (ADR 0016)

| Who | May |
|---|---|
| anyone on the LAN | `/screen/`, `/ws/screen`, `/assets/{id}`, `/api/session`, `/api/pair`, `/api/setup` (only while no PIN is set), `/healthz`, `/licenses` |
| paired remote (operator) | `/ws/remote` commands except setup ones, search, editor, `POST /api/assets`, `/api/library/export`, `GET /api/backup`, `GET /api/update/status`, `POST/DELETE /api/admin` |
| admin mode (PIN again, 15 min) | `/api/import/opensong`, `/api/library/merge`, `POST /api/update`, `POST /api/backup/restore`, `POST /api/assets/cleanup`, `/api/devices`, `/api/devices/{id}/revoke`, commands `SetScreenProfile`, `Calibrate`, `SetLogo`, `DeleteSong` |

Without a PIN nobody may control the screen: `/api/session` says `setup_needed`, and
`POST /api/setup { pin, name }` sets the PIN once (`invalid_pin` → 400, `already_set` → 409) and
pairs that browser with admin mode unlocked. Only the development flag `--open-without-pin` (used
by `just dev` and the tests) gives everyone admin rights while no PIN is set. Refusals: HTTP 401 (not paired) or 403 (needs admin
mode); WebSocket commands answer `Rejected { code: forbidden }`.

Application updates (ADR 0013, second amendment): `POST /api/update` takes a `.sursum-app` file
(with the `X-Sursum-Upload` header like other uploads) and answers `InstallOutcome`:
`installed { version }`, after which the server exits and its service manager starts the new
release, or `refused { reason }` with `no_key`, `not_supervised`, `bad_bundle`,
`already_current` or `failed` (nothing changed). `GET /api/update/status` answers
`UpdateStatus { build, release, on_trial, last, can_install }`; `last` is `committed { version }`
or `rolled_back { failed, restored, database_restored }`.

Whole-device backups (ADR 0018): `GET /api/backup` streams a `.sursum-backup` file (ZIP:
`manifest.json`, `sursum.db` without the PIN and paired devices, `assets/<id>`).
`POST /api/backup/restore` (admin mode, `X-Sursum-Upload`) answers `RestoreOutcome`:
`restored { summary: BackupSummary { created_unix, build, songs, sets, pictures } }`, after which
the server restarts and the next start puts the backup in place, or `refused { reason }` with
`not_supervised`, `not_a_backup`, `newer_version`, `too_large` or `failed` (nothing changed).
`POST /api/assets/cleanup` (admin mode) takes `PictureCleanupRequest { dry_run }` and answers
`PictureCleanup { pictures, bytes }`: what is (or would be) removed.

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
        | ShowNow { song_id }           // go to its next occurrence in the set, or insert it after
                                        // the live item; goes live
        | AddSong { song_id } | AddText { title, text }
        | RemoveItem { item_id } | MoveItem { item_id, to_index } | UndoRemove
        | NewSet { name } | ActivateSet { set_id } | RenameSet { set_id, name }
        | DeleteSet { set_id }          // refused with `set_on_screen` while one of its items is live
        | UndoDeleteSet                 // brings back the set deleted last, in its place
        | SetEndOfItem { mode: "gap"|"continue" }   // what Next does after an item's last slide
        | AddImage { title, asset_id } | AddBlank { title }      // pictures (POST /api/assets) and pauses
        | DisplayPower { action: "on"|"standby" }              // ADR 0012, runs outside the actor
        // admin mode only:
        | SetScreenProfile { profile } | Calibrate { on }      // ADR 0008
        | SetLogo { asset_id? } | DeleteSong { song_id }
```

## Server → client

```text
Welcome   { protocol, buildId, device?, serverTime }
Screen    { rev, live?: { deck, slideIndex, gap }, upcoming?: Deck, overlay, theme, transitionMs, logo?, layout: ScreenLayout }
Remote    { rev, program: Screen, next?: { deck, slideIndex, gap }, sections, activeSet?: SetView, sets,
            canUndo, displayPower: "unknown"|"on"|"standby"|"warmingUp"|"coolingDown"|{ unavailable: reason },
            screenProfile, screenConnected, nextStep: "slide"|"gap"|"item"|"start"|"end",
            endOfItem: "gap"|"continue" }
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
  `GoTo`, `GoToItemNumber`, `GoToSlide` and `ShowNow` also lift black or the logo (ADR 0004);
  relative steps keep them, so a slide can be prepared under black.
- **The gap**: with `endOfItem = gap` (the default), `Next` on an item's last slide shows an empty
  slide (`live.gap = true`, same item) before the next item; `Prev` leaves it. Not after a pause
  item; `NextItem`/`PrevItem` and absolute commands never stop in it.
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
