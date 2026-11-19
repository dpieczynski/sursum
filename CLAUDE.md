# sursum — guidance for Claude

Offline church lyrics display. A Raspberry Pi 5 next to the projector runs one Rust binary
(`sursum-server`) that holds all state and serves two embedded Leptos (CSR, WebAssembly) apps:
the **screen** (Chromium kiosk under cage, read-only) and the **remote** (Android tablet,
Linux/Windows PC). The same server also runs at home inside the desktop program `sursum`
(Windows, Linux; ADR 0020) for preparing songs. Out of scope: iPad, macOS, a Windows PC driving
the projector (ADR 0023). Everything is Rust; there is no Node/npm. Two churches of one parish are the
first users (Tulce, Gowarzewo). License: GPL-3.0-or-later.

Start of every session: read [docs/roadmap.md](docs/roadmap.md) (where we are) and skim
[docs/decisions/](docs/decisions/) (what is already decided — do not re-litigate without a
strong reason; if you have one, propose a new ADR). The roadmap's section S1 lists decisions that
are made but not built yet (one update path, ADR 0022; the WPE-on-DRM spike, ADR 0009): do not
extend what they remove. The planning document (Polish) is
[docs/plan-2026-11.md](docs/plan-2026-11.md); roadmap and ADRs win where they differ.

## Map

| Path | What |
|---|---|
| `crates/sursum-protocol` | **shared** serde types (WS messages, snapshots, `Deck`/`Slide`, `ScreenLayout`, REST DTOs); data only; native + wasm32 |
| `crates/sursum-core` | pure domain (server only): song model, slide compiler, program state machine, set ops, search, layout |
| `crates/sursum-formats` | OpenSong import (songs and sets) and the editor's plain-text form; OpenLyrics export is not built |
| `crates/sursum-server` | binary and library: axum, WebSocket, SQLite, auth, embedded frontend, display power, app and system updates, backups, platform |
| `crates/sursum-desktop` | binary `sursum` (ADR 0020): the server library on loopback plus one window (WebView2 through `tao`/`wry` on Windows, `gtk4`/`webkit6` on Linux); the window has no features |
| `crates/sursum-slide` | **shared** Leptos slide rendering for screen and remote preview (`Stage`, `LayeredStage`, `fit_deck`, the fixed text style `THEME`) |
| `crates/sursum-client` | **shared** Leptos WebSocket client (reconnect, heartbeat, status signal); the apps keep their own state |
| `apps/screen`, `apps/remote` | Leptos CSR apps built with Trunk |
| `e2e/` | WebDriver tests (thirtyfour) against the real binary |
| `image/` | device image (rpi-image-gen config and layers; immutable A/B, ADR 0013) |
| `deploy/` | units and kiosk files the image installs (`systemd/`, `kiosk/`); interim hand install (`pi-manual/`, goes with ADR 0022) |
| `library/public-domain/` | starter library of public-domain hymns in OpenSong format (`just library-zip`) |
| `assets/fonts/` | bundled OFL fonts with licenses |
| `fixtures/` | public-domain or synthetic test data only |
| `docs/` | architecture, protocol, roadmap, ADRs, formats, testing |

## Commands (from milestone M0; `justfile` is the source of truth)

`just dev` · `just build` · `just desktop` · `just test` · `just test-wasm` · `just e2e` ·
`just lint` · `just licenses` · `just cross-pi` · `just app-bundle <version>` ·
`just image <version>` · `just deploy <host> <version>`

## Hard rules

1. **All logic lives on the server** (parsing, slide splitting, order, navigation, program and
   preview, search, editor validation, screen layout). Browsers only draw and pick font sizes.
   Shared wasm crates share **data** (`sursum-protocol`) and **drawing** (`sursum-slide`),
   never domain logic — even though Rust would make that easy.
2. **`sursum-protocol` stays data-only**: serde types, no Leptos, no tokio, no logic; it must
   compile for both the native and the `wasm32-unknown-unknown` target.
3. **The screen never shows anything but content, black or logo**: no error UI, no cursor, no
   network resources, black background inline in `apps/screen/index.html` `<head>`. The one
   exception is the calibration test card while someone is calibrating (ADR 0008).
4. **Editing the set never changes the program.** The live deck is a frozen copy; see ADR 0005.
5. **No copyrighted song texts** in the repo, tests, docs or commit messages. Fixtures are
   public-domain or synthetic and listed in `fixtures/LICENSES.md`. Real libraries stay in `/local/`.
6. **Languages**: code, comments, commits, technical docs in English; UI in Polish through the
   compile-time `Msg` enum in `apps/remote/src/messages.rs` (never hard-coded strings;
   `leptos_i18n` is deferred until a second language is needed, ADR 0014 amendment).
7. **Licensing**: SPDX header `GPL-3.0-or-later` in every source file; dependencies must pass
   `cargo deny`; never copy code from other projects (OpenSong, FreeShow) without an ADR.
8. **Verify, don't recall**: check crate versions and APIs on crates.io and docs.rs before using
   them. Items marked ⚠ in the plan are unverified until a spike confirms them.

## Code principles (readability first)

- Pure functions in `sursum-core` (input → output, no I/O, no tokio): `compile_item`,
  `content_region`, `normalize`. Same for the fit algorithm in `sursum-slide`
  (`fit_font_size` takes a measuring closure; only the closure touches the DOM).
- Small single-purpose modules; domain names (`Program`, `Deck`, `ExclusionZone`), never
  `Manager`/`Helper`/`Utils`.
- Newtypes over bare values (`SongId`, `ItemId`, `SlideKey`, `Rev`); enums over boolean flags.
- No custom proc macros, no speculative generics. A trait only with ≥2 implementations or for
  test injection (`Platform`, `DisplayPower`).
- `thiserror` in libraries, `anyhow` only in `main`/CLI; errors carry context.
- `///` docs on every public item in core, protocol and slide, explaining *why*.
- Leptos components ≲150 lines; logic outside `view!` in plain functions; one signal-based store
  fed by the WebSocket; `web-sys` over hand-written `wasm_bindgen` externs; no `unsafe`.

## Definition of done

Tests added/updated and passing (`just test`, `just e2e` when UI changes) · `cargo fmt`,
`leptosfmt --check`, `clippy -D warnings` for native **and** `wasm32-unknown-unknown` · protocol
JSON snapshots reviewed if types changed · roadmap checkbox updated · ADR added if a decision was
made · commit message in English with sign-off (`-s`).

## Git

Remote: `git@github.com:dpieczynski/sursum.git`, default branch `main`. Work on feature branches
and pull requests. Commit or push only when the user asks.
