# WebSocket protocol

The types live in the shared crate `crates/sursum-protocol`, compiled by the server (native) and by
both Leptos apps (wasm32). There is no code generation: both sides use the same Rust types, so
they cannot drift apart within one build.

## Endpoints

- `GET /ws/screen`: read-only, no authentication, loopback and LAN only.
- `GET /ws/remote`: requires the device cookie set during PIN pairing; `Origin` and `Host` are
  validated on upgrade (protection against cross-site WebSocket hijacking and DNS rebinding).
- REST under `/api/…` for large or on-demand data: search, song drafts and preview, import,
  library export/merge, pairing, the device list and application updates.

### Rights (ADRs 0016, 0024)

| Who | May |
|---|---|
| anyone on the LAN | `/screen/`, `/ws/screen`, `/assets/{id}`, `/api/session`, `/api/pair`, `/api/setup` (only while no PIN is set), `/healthz`, `/healthz/trial` (503 unless the state actor answers and the kiosk's screen is connected; the system update's trial checks it), `/licenses` |
| paired remote | everything else: `/ws/remote` and all its commands, `/api/songs*`, `/api/assets`, `/api/assets/cleanup`, `/api/library/export`, `/api/library/merge`, `/api/import/opensong`, `/api/backup`, `/api/backup/restore`, `/api/update/status`, `/api/system/update`, `/api/devices`, `/api/devices/{id}/revoke` |

There is one level of rights (ADR 0024): a paired remote may do everything. Without a PIN nobody
may control the screen: `/api/session` says `setup_needed`, and `POST /api/setup { pin, name, church }`
sets the PIN once (`invalid_pin` or `church_missing` → 400, `already_set` → 409), names the
device's church (ADR 0025) and pairs that browser. Only a server on a loopback address runs
without a PIN (the *local* access mode, below). Refusals: HTTP 401 (not paired); WebSocket commands answer
`Rejected { code: forbidden }`, after which the remote reloads into the pairing form. Revoking a
device closes its WebSocket and refuses its reconnects with 401, so an idle remote gets no
`forbidden`: after every lost or refused connection it asks `/api/session` and reloads into the
pairing form when that says `paired: false` (a network error leaves it waiting to reconnect).

The desktop program (ADR 0020) runs the server in the *local* access mode: it listens on loopback
only (the server refuses to start otherwise), there is no PIN and no pairing, and every request is
allowed. The development flag `--open-without-pin` (used by `just dev` and the tests) is the same
mode on a device installation (with a screen). `/api/session` then says `"installation": "library"` (a device says `"screen"`; older
servers send nothing, which reads as `"screen"`), and the remote offers songs, the editor, sets
for any church (a church chosen with `UseChurch` decides which sets are listed and whose new sets
are; before one is chosen every set is listed), the library file and the backup download
(restoring needs a service manager). The same mode is reachable as `sursum-server --library` for the
browser tests.

Whole-device backups (ADR 0018): `GET /api/backup` streams a `.sursum-backup` file (ZIP:
`manifest.json`, `sursum.db` without the PIN and paired devices, `assets/<id>`).
`POST /api/backup/restore` (`X-Sursum-Upload`) answers `RestoreOutcome`:
`restored { summary: BackupSummary { created_unix, build, songs, sets, pictures } }`, after which
the server restarts and the next start puts the backup in place, or `refused { reason }` with
`not_supervised`, `not_a_backup`, `newer_version`, `older_version` (made before ADR 0025;
not converted), `too_large` or `failed` (nothing changed).
Updates (ADR 0019; the only kind since ADR 0022): `POST /api/system/update` (`X-Sursum-Upload`)
takes a `.sursum-system` file and answers `InstallOutcome`: `installed { version }`, after which
the device copies the database, writes the other slot and restarts into it on trial, or
`refused { reason }` with `no_key`, `not_supervised` (not a device built from the image),
`bad_bundle` (not a valid, signed update), `already_current`, `busy` (another update is installed
or on trial) or `failed` (nothing changed). `GET /api/update/status` answers
`UpdateStatus { build, system }`: `system` is `SystemStatus { version, on_trial, installing,
target, last, can_install }` on devices running the image, absent elsewhere; `last` is
`committed { version }` or `rolled_back { failed, database_restored }`. `build` differs from
`system.version` only while a developer's build runs (`just try`).
`POST /api/assets/cleanup` takes `PictureCleanupRequest { dry_run }` and answers
`PictureCleanup { pictures, bytes }`: what is (or would be) removed.

JSON text frames (`serde_json` on both sides, readable in browser dev tools); serde enums tagged
with `"t"`; default Rust (snake_case) field names, since no JavaScript consumes them.

## Client → server

```text
Hello   { protocol, role: "screen"|"remote", client_id, build_id }
Cmd     { seq, base_rev, cmd: Command }
Ping    { }

Command = Next | Prev | NextItem | PrevItem                // relative: subject to `Stale`
        | GoTo { item_id, slide_index } | GoToItemNumber { number } | GoToSlide { slide_index }
        | SetOverlay { overlay: "none"|"black"|"logo" } | ToggleBlack
        | ShowNow { song_id }           // go to its next occurrence in the set, or insert it after
                                        // the live item; goes live
        | AddSong { song_id } | AddText { title, text }
        | RemoveItem { item_id } | MoveItem { item_id, to_index } | UndoRemove
        | NewSet { name, date? } | ActivateSet { set_id } | RenameSet { set_id, name }
        | DateSet { set_id, date? }     // date: YYYY-MM-DD or YYYY-MM-DDTHH:MM (ADR 0025)
        | CopySet { set_id, name, date?, church? }   // "Nowy zestaw z tego": a new set, active
        | DeleteSet { set_id }          // refused with `set_on_screen` while one of its items is live
        | UndoDeleteSet                 // brings back the set deleted last, in its place
        | SetEndOfItem { mode: "gap"|"continue" }   // what Next does after an item's last slide
        | AddImage { title, asset_id } | AddBlank { title }      // pictures (POST /api/assets) and pauses
        | DisplayPower { action: "on"|"standby" }              // ADR 0012, runs outside the actor
        | SetScreenProfile { profile } | Calibrate { on }      // ADR 0008
        | SetLogo { asset_id? } | DeleteSong { song_id }
```

## Server → client

```text
Welcome   { protocol, build_id }
Screen    { state: ScreenState { rev, live?: { deck, slide_index, gap }, upcoming?: Deck, overlay,
            transition_ms, layout: ScreenLayout, calibration?: ScreenProfile, logo? } }
Remote    { state: RemoteState { rev, program: ScreenState, next?, active_set?: SetView, sets,
            can_undo, display_power, screen_profile, screen_connected,
            next_step: "slide"|"gap"|"item"|"start"|"end", end_of_item: "gap"|"continue" } }
CmdResult { seq, outcome: Applied{rev} | Stale{rev} | Rejected{code} }
Pong      { }
```

`Rejected.code` is one of `not_found`, `no_active_set`, `forbidden`, `unsupported`,
`invalid_profile`, `display_unreachable`, `display_busy`, `display_auth_failed`, `set_on_screen`;
the remote words them in Polish. There is no `Notice` message and no server or client time in
the handshake. How text looks (font, colour, size range) is not on the wire: it is one fixed style
in `sursum-slide` (`theme.rs`).

**Changing the snapshot types** (removing a field, or adding one without `#[serde(default)]`): a
screen page from the previous build cannot read the new snapshots, keeps its last slide and,
because it reloads only at a moment with no text on screen, never notices. On a device the
kiosk must restart once after such an update (a system update or a reboot does it). Removed on
2026-11-19: `ScreenState.theme`, `RemoteState.sections`, `ClientMsg::Log`; on 2026-11-20:
`Deck.background` and the image `fit` (pictures are always whole, on black).

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
- **Song edits** go through REST (the editor). A draft carries `base_version`, the version the
  editor loaded (ADR 0025); saving builds a new version on it and answers
  `Saved { song_id, version }`, or `Conflict` if that version was replaced meanwhile (another
  remote saved). A version that lost a merge can still be saved on; the song then keeps both
  versions until review.
- **Library files** (`GET /api/library/export`, `POST /api/library/merge[?dry_run=true]`):
  `.sursum` format 2, a ZIP with `manifest.json` (format and counts), `songs.jsonl`,
  `churches.jsonl` and `sets.jsonl`, one document per line with its heads
  (`{ id, heads: [{ body, ancestors }] }`, `body` null for a deletion), and `assets/<id>`, the
  pictures the sets use. Version ids are computed by the reader, never read; a picture is
  stored only if its bytes give its id. Every set travels, also other churches' (kept and passed
  on, not shown). The export is written to a temporary file and streamed with its length; the
  upload is streamed to a temporary file too (up to the picture store's limit plus 256 MB), so
  neither is held in memory. `MergeReport { added, updated, unchanged, kept_local, deleted,
  conflicts, sets_added, sets_updated, sets_to_review, other_church_sets, pictures_added }`:
  `conflicts` and `sets_to_review` are the titles and names of documents changed in two places
  (one shown, the other kept for review); a dry run counts the pictures it would add. Format 1
  files are refused.
- **OpenSong import** (`POST /api/import/opensong`) answers `ImportReport { inserted, updated,
  unchanged, failed, sets, to_review, problems }`; `to_review` names songs whose file text met a
  correction made in sursum since the last import (both versions stay, ADR 0025).
- **Churches** (ADR 0025): `RemoteState.church` is the church this device serves (named at
  setup; in the desktop program the church whose sets it lists, `None` until one is chosen),
  `RemoteState.churches` every church it knows; commands `NameChurch { name }` (create or
  rename) and `UseChurch { church_id }` (join a church learned from a file; in the desktop
  program, choose whose sets to prepare). Library files carry them in `churches.jsonl`.
- **Review** of songs changed in two places: `RemoteState.songs_to_review` counts them and
  `SongSummary.review` marks them in search results; `GET /api/songs/review` lists them,
  `GET /api/songs/{id}/versions` answers `SongVersions` (every live version in the editor's text
  form, the shown one first), and `POST /api/songs/{id}/keep { version }` keeps one version,
  replacing the others (`SaveOutcome`; `Conflict` if that version is gone).
- **Review** of sets changed in two places: `SetSummary.review` and `SetView.review` mark them;
  `GET /api/sets/{id}/versions` answers `SetVersions` (every live version with its name, date and
  items, the shown one first; `SetVersionItem.missing_elsewhere` marks items another version
  lacks), and the command `KeepSetVersion { set_id, version, add }` keeps one version with the
  items `add` from the others, each after the item it follows there (or at the end); `NotFound`
  if that version is gone. Like any set edit it never changes the program (ADR 0005).
- **Display power** commands are executed asynchronously; failures return
  `Rejected(DisplayUnreachable | DisplayBusy | DisplayAuthFailed)` and never block navigation.

## Reconnect

`Hello` → `Welcome` + full snapshot. The screen compares `deck.deckId` and the slide key with what
it shows; if equal, it redraws nothing (no transition). Backoff 250 ms → 5 s with jitter; ping
every 5 s; no pong for 15 s → reconnect. A different `protocol` or `buildId` makes the page reload
(the screen waits until it shows an overlay or the program changes).

## Type notes

- In `sursum-protocol`: messages, commands, snapshots, `Deck`/`Slide`, `Theme`, `Overlay`,
  `ScreenLayout`, `ScreenProfile`, `Rect`, `DisplayPowerState`, REST DTOs, error codes.
  Not in it (server only): DB rows, parser AST, the internal `Program`, any logic.
- The crate depends only on `serde` (plus e.g. `uuid` with the `serde` feature) so it compiles for
  `wasm32-unknown-unknown` without tokio or `getrandom`; CI builds it for both targets.
- `insta` JSON snapshots of every message guard the wire format: after an update, a screen may
  still run old code until it reloads, so field renames must be deliberate.
- The pseudo-code above is abridged; the Rust structs are authoritative.
