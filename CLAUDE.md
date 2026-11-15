# sursum — guidance for Claude

Offline church lyrics display. A Raspberry Pi 5 next to the projector runs one Rust binary
(`sursum-server`) that holds all state and serves two embedded Svelte apps: the **screen**
(Chromium kiosk under cage, read-only) and the **remote** (Android tablet, Linux/Windows PC).
Two churches of one parish are the first users (Tulce, Gowarzewo). License: GPL-3.0-or-later.

Start of every session: read [docs/roadmap.md](docs/roadmap.md) (where we are) and skim
[docs/decisions/](docs/decisions/) (what is already decided — do not re-litigate without a
strong reason; if you have one, propose a new ADR). The original plan (Polish) is
[docs/plan-2026-11.md](docs/plan-2026-11.md); it is a historical record, not kept up to date.

## Map

| Path | What |
|---|---|
| `crates/sursum-core` | pure domain: song model, slide compiler, program state machine, set ops, search, screen layout, protocol types (ts-rs) |
| `crates/sursum-formats` | OpenSong import, OpenLyrics export |
| `crates/sursum-server` | binary: axum, WebSocket, SQLite, auth, embedded frontend, platform |
| `frontend/packages/{protocol,render,i18n}` | generated types + WS client; shared slide rendering; messages |
| `frontend/apps/{screen,remote}` | audience screen; operator remote |
| `e2e/` | Playwright tests against the real binary |
| `deploy/` | systemd units, Pi setup, Windows pilot script |
| `fixtures/` | public-domain or synthetic test data only |
| `docs/` | architecture, protocol, roadmap, ADRs, formats, testing |

## Commands (from milestone M0; `justfile` is the source of truth)

`just dev` · `just build` · `just test` · `just e2e` · `just bindings` · `just lint` ·
`just cross-pi` · `just cross-win` · `just deploy <host>`

## Hard rules

1. **All logic lives on the server** (parsing, slide splitting, order, navigation, program and
   preview, search, editor validation, screen layout). Browsers only draw and pick font sizes.
   Nothing is implemented twice in two languages.
2. **Never edit generated files** in `frontend/packages/protocol/src/generated/`; change the Rust
   types and run `just bindings`.
3. **The screen never shows anything but content, black or logo**: no error UI, no cursor, no
   network resources, black background inline in `<head>`.
4. **Editing the set never changes the program.** The live deck is a frozen copy; see ADR 0005.
5. **No copyrighted song texts** in the repo, tests, docs or commit messages. Fixtures are
   public-domain or synthetic and listed in `fixtures/LICENSES.md`. Real libraries stay in `/local/`.
6. **Languages**: code, comments, commits, technical docs in English; UI in Polish via i18n.
7. **Licensing**: SPDX header `GPL-3.0-or-later` in every source file; dependencies must pass
   `cargo deny`; never copy code from other projects (OpenSong, FreeShow) without an ADR.
8. **Verify, don't recall**: check crate/npm versions and APIs on crates.io, docs.rs, npm before
   using them. Items marked ⚠ in the plan are unverified until a spike confirms them.

## Code principles (readability first)

- Pure functions in `sursum-core` (input → output, no I/O, no tokio): `compile_deck`,
  `content_region`, `normalize`.
- Small single-purpose modules; domain names (`Program`, `Deck`, `ExclusionZone`), never
  `Manager`/`Helper`/`Utils`.
- Newtypes over bare values (`SongId`, `ItemId`, `SlideKey`); enums over boolean flags.
- No custom proc macros, no speculative generics. A trait only with ≥2 implementations or for
  test injection (`Platform`, `Clock`).
- `thiserror` in libraries, `anyhow` only in `main`/CLI; errors carry context.
- `///` docs on every public item in core, explaining *why*.
- Svelte components ≲150 lines; logic in plain `.ts` modules; one store fed by the WebSocket.

## Definition of done

Tests added/updated and passing (`just test`, `just e2e` when UI changes) · `clippy -D warnings`,
`svelte-check`, eslint, prettier clean · bindings up to date · roadmap checkbox updated · ADR
added if a decision was made · commit message in English with sign-off (`-s`).

## Git

Remote: `git@github.com:dpieczynski/sursum.git`, default branch `main`. Work on feature branches
and pull requests. Commit or push only when the user asks.
