# Roadmap

Update the checkboxes as work lands. Acceptance criteria are the definition of done for each
milestone. ⚠ = assumption to verify in a spike before relying on it.

## S1 — Simplification (decided 2026-11-19)

An audit looked for decisions that cost more code than they return. Six were changed; the work
below comes before new features. Order: CI image build, one update path, engine spike.

- [x] Platform scope (ADR 0023): no iPad, no macOS program, no Windows parish PC. Removed: the
  iPad Simulator workflow, the `safari-ipad` engine in the e2e harness,
  `deploy/windows/start-pilot.ps1`, `just cross-win`, `sursum-server.exe` in CI and releases
- [x] Public repository (ADR 0011 amendment): history checked for secrets and song texts
  (2026-11-19, nothing found); visibility switched by the maintainer the same day
- [ ] Apply for free signing of the Windows program (SignPath Foundation ⚠) to end Defender
  deleting the download
- [x] One level of rights (ADR 0024, 2026-11-19): admin mode removed (second PIN entry,
  `/api/admin`, `SessionInfo.admin`/`admin_remaining_s`, the countdown, the locks in Ustawienia,
  the per-device lockout); pairing, the PIN and revocation stay. Server and e2e tests rewritten;
  `just test`, `just test-wasm` and `just e2e` (Chromium, WPE, WebKitGTK) pass; the guide's
  Ustawienia screenshot retaken
- [x] Screen profile only from the calibration panel (ADR 0008 amendment, 2026-11-19):
  `[screen_profile]` in `config.toml` is ignored with a warning; e2e tests set zones with
  `SetScreenProfile`
- [x] Dead code removed (2026-11-19): `RemoteState.sections` and `Program::sections`,
  `ClientMsg::Log` and `sursum_client::report`, `ScreenState.theme` with `Theme` and `TextAlign`
  (the fixed style is `sursum-slide/src/theme.rs`), `.devcontainer/`. A screen page of an older
  build cannot read the new snapshots: the pilot Pi's kiosk needs one restart after updating
  (`docs/protocol.md`)
- [ ] Image and `.sursum-system` built by CI on an arm64 runner (the M4 and M7 items below);
  the release workflow attaches them
- [ ] One update path (ADR 0022): `sursum.service` runs the system's `/usr/lib/sursum/sursum-server`;
  the database copy and its restore move into the system trial (closes the M7 item about shared
  state); remove `.sursum-app`, `bundle install|rollback|guard`, `releases/`, `POST /api/update`,
  the program half of the remote's update views, `just app-bundle`, `just deploy`,
  `deploy/pi-manual/`; decide how a developer tries a server build on a device ⚠; reflash the
  pilot Pi with a device backup; rewrite `docs/releasing.md` and the guide's update section
- [ ] WPE-on-DRM spike (ADR 0009 amendment of 2026-11-19): rebuild sid's `wpewebkit` 2.54 for
  trixie arm64 in CI; spike image with `WPE_PLATFORM=drm MiniBrowser --fullscreen
  --bg-color=black` and no cage, sent to the pilot Pi as a system update; check start without a
  compositor, forced 1080p60 after a late TV and a replug, no cursor, web process death, boot to
  first frame, RAM after 8 h, crossfade, system size. Then an ADR: switch (and delete cage,
  seatd, `wlr-randr`, `kiosk-session`, the cursor theme, the policies; move the WPE tests to
  2.54 with MiniBrowser) or stay on Chromium for good (and delete the WPE tests and cog)
- [ ] Smaller findings of the audit, not decided, cheapest first: `ImageFit::Cover` is never
  chosen and `Background` has one variant (both in stored set items and decks); the demo set
  comes back whenever no set exists (fixed in Y1.6); the "build the web apps with one build id"
  block is copied into each workflow; `OpenForDevelopment` and `LocalOwner` are both "no PIN,
  everything allowed"; `.sursum` and `.sursum-backup` overlap (and the backup restore swaps the
  database file, which ties it to systemd)

## Y1 — Synchronizing songs and sets (ADR 0025)

A song or a set keeps its identity and gets versions; when it was changed in two places the
version with more changes is current on every device and the other waits on a review list. Sets
belong to a church, have a date and travel with their pictures in the same `.sursum` file;
conflict copies ("kopia ze scalenia") go away. No migration of existing data. Plan and work
packages: [plan-sync.md](plan-sync.md).

- [x] Y1.0 ADR 0025 and the plan (2026-11-19)
- [x] Y1.1 (with the song part of Y1.2): versions in `sursum-core` with property tests
  (`versions.rs`); schema numbering restarted above 100, databases and device backups of earlier
  builds refused (`older_version`); songs saved, loaded and deleted through versions
  (`db/songs.rs`, bodies as JSON with sorted keys, whatever `serde_json` features the build
  has); the editor sends `base_version` and gets `Conflict` if it was replaced; `.sursum`
  format 2 for songs with merge and report; the two-church scenario, the text typed back, a
  deletion against a change and stale files as tests; conflict copies removed (2026-11-19).
  Not yet run on the pilot Pi (its data folder must be emptied first)
- [ ] Y1.2 rest: the merge upload streamed to disk (moved to Y1.6, when pictures make files
  large)
- [x] Y1.3 Review of songs changed in two places (2026-11-20): `songs_to_review` in the
  remote's state, a mark in search results, the "Pieśni do przejrzenia" line in the Zestaw and
  Pieśni views, the comparison of versions with "Zostaw tę" (`/api/songs/review`,
  `/api/songs/{id}/versions`, `/api/songs/{id}/keep`); server and e2e tests (Chromium,
  WebKitGTK). Editing a version that lost before keeping one is not offered: keep one, then edit
- [x] Y1.4 OpenSong import under the same rules (2026-11-20): an import builds on what the
  previous import of the song made on this device, or on a first version shared by every
  device; importing an old folder again never undoes a correction nor brings back a deleted
  song; a new text next to a correction made in sursum waits for review
  (`ImportReport.to_review`)
- [x] Y1.5 Churches (2026-11-20): a church is a versioned document (`db/churches.rs`) whose
  id comes from its first name (`sursum-core::churches::church_id`, folded like the search);
  the first-start form asks for the church's name (`PairRequest.church`, `church_missing`);
  `RemoteState.church` and `churches`; commands `NameChurch` (create or rename) and `UseChurch`;
  Ustawienia → Kościół; churches travel in `.sursum` files (`churches.jsonl`). Sets are not yet
  tied to churches (Y1.6)
- [x] Y1.6a Sets as versions (2026-11-20): every set change saves a version (`db/sets.rs`, the
  old set tables dropped); sets carry an optional date (`NewSet.date`, `DateSet`,
  `is_set_date`) and their church; the list shows drafts first, then by date, latest on top;
  "Nowy zestaw z tego" (`CopySet`: a new set, new item ids, becomes active); a device shows its
  church's sets (and those made before the church was named, which naming adopts); the demo set
  is made once, on the first start, with an id of its own; imported OpenSong sets are per church
- [ ] Y1.6b Sets and their pictures in the `.sursum` file; the merge upload streamed to disk
- [ ] Y1.6c Set review: sets changed in two places, the comparison with "Dodaj" 
- [ ] Y1.7 Zestaw view in the desktop program with the church switcher; "Przywróć z kopii" hidden
  there
- [ ] Y1.8 Operator guide

## M0 — Skeleton

- [x] Git repository, `.gitignore`, `.gitattributes`, `.editorconfig`
- [x] License (GPL-3.0-or-later), README, CONTRIBUTING (DCO), `fixtures/LICENSES.md`
- [x] Guidance files (`CLAUDE.md` ×4), docs, ADRs
- [x] ~~Devcontainer definition~~ (removed 2026-11-19: never built) (Debian trixie: rustup + `wasm32-unknown-unknown`, trunk 0.22, wasm-bindgen-cli 0.2.130, binaryen, leptosfmt, chromium + chromium-driver, cog + wpewebkit-webdriver; no Node/Python). Not yet built/tried in VS Code; zig + cargo-zigbuild still to add
- [x] Browsers image for e2e (`e2e/browsers.Containerfile`, podman, host network): Chromium 156 + chromedriver, cog 0.18.4 + WPEWebDriver (WPE WebKit 2.48.3) + Mesa; since P2 also WebKitGTK 6.0 with its WebDriver under Xvfb
- [x] Cargo workspace: `sursum-protocol`, `sursum-core`, `sursum-formats`, `sursum-server`, `sursum-slide`, `sursum-client`, `apps/screen`, `apps/remote`, `e2e`; `default-members` = native crates; `rust-toolchain.toml`, `deny.toml`
- [x] Leptos 0.8 CSR apps with Trunk (`index.html`, `Trunk.toml` with `/ws` proxy; WebSocket upgrade through `trunk serve` verified)
- [x] rust-embed serving both `apps/*/dist`; `allow_missing` verified (server compiles without a frontend build; `/screen/` then serves a black page)
- [x] Bundled OFL fonts in `assets/fonts/` with `LICENSES.md` (Lato 400/700, latin + latin-ext)
- [x] `justfile`, SPDX check script, `deny.toml` (licenses, bans, sources, advisories pass)
- [x] Third-party license page (`cargo about` + fonts) at `/licenses`, linked from the remote
- [x] First WebDriver tests (thirtyfour 0.38 + chromedriver in the container)
- [x] Same tests on WPE WebKit: capability `wpe:browserOptions { binary: "cog", args: ["--automation", "--platform=headless", "--bg-color=black"] }` verified; WPE needs Mesa (`libgles2`, `libegl1`, `libgl1-mesa-dri`) or its renderer crashes
- [x] Cross-compilation verified locally with zig 0.17.1 + cargo-zigbuild 0.24.1: `aarch64-unknown-linux-gnu.2.36` and `x86_64-pc-windows-gnu` (both 11 MB with the apps embedded)
- [x] GitHub Actions CI, running on GitHub since 2026-11-16 (`just test-wasm` added 2026-11-16) (fmt, leptosfmt, clippy native + wasm32, `cargo test`, wasm-bindgen-test, trunk build + WASM size check, WebDriver e2e (Chromium; screen subset also on WPE, blocking), cargo-deny, cross builds)

**Done when:** `just dev` gives auto-reload for both apps with a working WS echo using
`sursum-protocol` types; `just build` produces one binary serving both apps; the first WebDriver
test passes in Chromium and WPE; CI is green.

## M1 — Screen, remote and one song on a laptop

- [x] `sursum-protocol`: messages and snapshots with `insta` JSON snapshots
- [x] core: song model, slide compiler, program state machine (next/prev/goto/overlay, Stale rule, frozen deck), PD + synthetic demo songs
- [x] Public-domain starter library: 215 Polish and Latin hymn texts from Mioduszewski's *Śpiewnik kościelny* (1838–1849) and *Pastorałki i kolędy* (1843) in `library/public-domain/` (OpenSong, license in `<copyright>` and hidden `;` comments, `LICENSES.md`, `REJECTED.md`), `just library-zip` for the remote import; doubtful readings listed for a human check
- [x] core: `content_region` (exclusion zones) and `ScreenLayout` in the snapshot; profile from `config.toml`
- [x] server: state actor, `watch` fan-out, `/ws/screen`, `/ws/remote`, Origin check, local-network guard, `runtime_state` with `boot_id` (migrations hand-rolled with `PRAGMA user_version`: `rusqlite_migration` 2.7 requires rusqlite 0.41)
- [x] `sursum-slide`: `Stage`, `LayeredStage` (A/B + overlay + masks), `fit_deck` (pure algorithm + DOM measuring), local fonts
- [x] `sursum-client`: WebSocket with reconnect (backoff + jitter), heartbeat, Hello/Welcome, build-id reload. No stale-`rev` dropping: messages arrive in order per connection and server revisions restart after a server restart, so clients apply every snapshot
- [x] remote: current/next preview, next/prev, item prev/next, section chips (now slide tiles, ADR 0017), set list, BLACK/LOGO, pedal keys (PgDn/PgUp/arrows/space/B). UI texts via a compile-time message enum (`apps/remote/src/messages.rs`); `leptos_i18n` deferred until a second language is needed
- [ ] On-site display power spike: `cec-ctl` on both displays (on / standby / power status), projector LAN (PJLink) and RS-232 check in Tulce; record result in ADR 0012
- [ ] Hardware spike on a Pi 5: cage + Chromium ran on the pilot Pi 5 (2026-11-17, see M4: flags and policies ✓, cursor ✓ transparent theme, forced 1080p60 ✓; first boot showed black, then the logo). Still open: white flash ⚠ and crossfade smoothness checked with an HDMI capture, and a first look at the Tulce pillar. The WPE comparison waits for a WPEPlatform launcher (cog is legacy, ADR 0009 amendment)
- [x] Screen WebDriver tests pass on WPE locally; cross-engine fit test (WPE screen vs Chromium remote) passes. Blocking in CI once CI runs

**Done when:** WebDriver tests pass (black first frame; last slide stays on disconnect; reconnect
without a transition; one font size per song; preview matches screen font size and line count;
text never enters an exclusion zone); manual run on a Linux laptop + Android tablet.

Status 2026-11-16: all listed WebDriver tests pass on Chromium and (screen) WPE WebKit; the
manual run on a laptop + tablet and both hardware spikes are still open.

## M2 — OpenSong import, library, search, editor

- [x] `sursum-formats`: OpenSong songs, import report, idempotent re-import; CLI `sursum-server import-opensong <Songs dir>` (875 real songs, 0 failures)
- [x] ZIP upload of an OpenSong folder from the browser (`POST /api/import/opensong`, backup first, library reloaded, one-line report in the remote)
- [x] Extended OpenSong conventions checked against a real Polish library (see [opensong-format.md](opensong-format.md)); numbered lines still ⚠
- [x] SQLite schema (migration 2: `songs` with revisions, `song_history`; replaced by the `versions` table, ADR 0025)
- [x] Backups (`VACUUM INTO`, 14 kept, at startup and before imports), `quick_check` at startup + automatic restore of the newest backup (damaged file kept in `corrupt/`)
- [x] Whole-device backup (ADR 0018): `.sursum-backup` with the database (without PIN and paired devices) and all pictures, download from any remote, restore in admin mode staged and applied at the next start (device keeps its own credentials, replaced database kept in `backups/`); unused pictures removed only on request (admin, count first, hold to confirm)
- [x] In-memory search with Polish folding (ł!) and ranking; ~1.2 ms per query over 5000 songs (laptop); remote search panel with Show now / Add / Edit
- [x] Keyboard-friendly song editor (OpenSong markup, `---` optional break) with server-compiled preview drawn by `sursum-slide`; revision history and optimistic locking (conflict detection)

**Done when:** every convention has a synthetic fixture test; the real parish library imports
locally without critical errors; search by number/title/fragment works regardless of diacritics;
p95 < 30 ms for 5000 synthetic songs (laptop) and measured on the Pi; a new song can be entered
without a mouse.

## M3 — Sets, live editing, pairing, shared library

- [x] Persistent sets (create, activate), live editing (move, hold-to-remove, undo), frozen program with ghost position, "Show now", announcements (text items), the parish logo for the LOGO button (admin upload, drawn inside the content area, pre-decoded), pictures (content-addressed uploads in `<data>/assets/`, PNG/JPEG/WebP, decoded before the fade) and pauses (blank items); pictures travel in whole-device backups (ADR 0018), not in `.sursum` files
- [x] Remote: two-column landscape layout (previews left, library/set right, controls bottom)
- [x] Remote: "#" keypad to jump to item N of the set (plan §7), no system keyboard needed
- [x] OpenSong set import (folder and ZIP): songs by path/file name/title, `presentation` override, custom and scripture groups as text items, image groups reported; re-import replaces the set
- [x] Multi-remote `Stale` rule (M1)
- [x] PIN pairing: `sursum-server set-pin`, salted hash, rate-limited `/api/pair` with doubling lockout, `HttpOnly; SameSite=Strict` device cookie (only its hash stored), protected `/ws/remote` and `/api/*`, pairing screen in the remote; screen stays open; a device without a PIN is closed and its first remote sets the PIN (`/api/setup`, ADR 0016 amendment), `--open-without-pin` for development only
- [x] Roles (admin mode removed 2026-11-19, ADR 0024): paired remote = operator; admin mode by entering the PIN again (15 min, memory only) for import, merge, calibration and the device list; revocation closes the remote's session at once (ADR 0016)
- [x] Deleting songs (admin, hold in the editor): soft delete with history kept; tombstones travel in `.sursum` files and delete only the same or an older version on the other device; histories from merged files are remembered, so stale files never resurrect or duplicate songs; merge and import run as one transaction
- [x] `.sursum` library file (merge rules superseded by ADR 0025, roadmap Y1): export (`GET /api/library/export`, ZIP with `manifest.json` + `songs.jsonl` incl. revision history), merge with mandatory dry run in the remote; fast-forward by ancestor hashes, newer-local kept, conflicts stored as a copy, backup before merging (sets, the logo and pictures are not carried yet; songs and their deletions are)
- [x] Screen calibration panel: hold to open, test card on the screen and the tablet, drag a zone or type exact numbers, safe area margins; stored in settings, validated on the server (ADR 0008 amendment)

**Done when:** WebDriver tests prove set edits never change the screen; simultaneous "next" from two
remotes advances once; merging two libraries with a planted conflict yields a correct report and
no data loss; the pillar zone can be set from the remote in < 2 minutes.

## P1 — Windows pilot (dropped, ADR 0023)

Closed on 2026-11-19 without a run: the device image works on the pilot Pi 5, so the first
services run on the Pi with the OpenSong PC as plan B. `deploy/windows/start-pilot.ps1` (written,
never run on Windows) is removed.

- [x] Operator guide in Polish: `docs/operator-guide-pl.md`

## M4 — Raspberry Pi deployment

- [ ] Device image with rpi-image-gen (ADR 0013): `trixie-minbase-ab` + `image-rota`, layers `sursum-kiosk` and `sursum-device`, slot-shared `/var/lib/sursum`, pinned tool version. Written (`image/`, `just image <version>`, rpi-image-gen v2.9.0 in a podman container; ADR 0013 amendment of 2026-11-17: SSH key only, Ethernet only, preferred resolution, black boot) first build (about 12 min through QEMU) and first boot on a Pi 5 checked (2026-11-17): black screen then logo, all services up, release 0.1.2 committed after its trial, read-only root, kiosk localhost only, firewall, 64M journal, device backup restored. `sursum.local` works (Chromium on Linux, Android without a VPN); it fails in a Flatpak Firefox (no mDNS in the sandbox) and with Tailscale's DNS on, noted in the operator guide. Open: image is Pi 5 only (a Pi 4 blinks 7 times: no kernel), diagnostic boot switch (`just image <version> y`: firmware rainbow, kernel and systemd messages instead of black) ⚠ not yet built or booted. The data partition now grows to fill the card before its first mount (`sursum-grow-persistent`; checked on the Pi 5: 512M image partition grew to 26 GB on a 32 GB card, the second boot was a no-op; image 4 GB instead of 7.8)
- [x] Server side of the screen guard (plan §11.3): `[screen_guard] restart_command = [...]`, `grace_s` (default 15 s; 60 s after startup) in `config.toml`; only a screen on this machine (loopback: the kiosk) counts; remotes warn while none is connected
- [x] Screen guard on the device: `/etc/sursum/config.toml` sets `restart_command = ["touch", "/run/sursum/restart-kiosk"]`; `sursum-kiosk-restart.path` restarts the kiosk as root (no polkit or sudo in the image; the server stays unprivileged). The server reads the image's `/etc/sursum/config.toml` (`SURSUM_SYSTEM_CONFIG`) before the data directory's `config.toml` checked on the Pi (2026-11-17): renderer killed, the guard asked 15 s later and the kiosk restarted
- [x] Server side of the systemd watchdog: `Type=notify` readiness and `WATCHDOG=1` pings only while the state actor answers (checked under systemd on a desktop); `deploy/systemd/sursum.service` is the unit the image installs
- [x] In the image (first item above, booted on the Pi 5): systemd units, silent black boot (no splash image, decided 2026-11-17), forced HDMI mode (instead of a fallback EDID file), nftables, Avahi, the screen guard as the kiosk watchdog, hardware and systemd watchdogs
- [x] Screen resolution, decided 2026-11-17 (ADR 0013 amendment): HDMI0 forced to 1080p60 (`video=HDMI-A-1:1920x1080@60D` in the kernel command line keeps the connector on; cage ignores it for its mode, so `deploy/kiosk/kiosk-session` sets and keeps `SURSUM_SCREEN_MODE=1920x1080@60` with `wlr-randr`; checked on the Pi with an LG 4K TV: runtime switch from 3840×2160@30 to 1920×1080@60 works). Image v0.1.0-36: cage sets the TV's 4K30 at start and kiosk-session switches to 1080p60 before Chromium starts (the page only ever shows in 1080p) checked on the Pi with the LG TV (2026-11-17): 1920×1080@60 stays after the TV is switched off and on and after an HDMI replug (cage kept the mode; kiosk-session is the safety net), CEC comes back too
- [x] Release workflow for `v*` tags (`.github/workflows/release.yml`, [releasing.md](releasing.md)): signed Pi bundle checked against the compiled-in key, Windows binary, GitHub release ⚠ not yet run (needs the first tag)
- [ ] Image build in CI on arm64 ⚠ (runner availability, privileged container); artifacts `.img` and `.sursum-system` (ADR 0019; rpi-image-gen's `.update.tar.zst` is not used)
- [x] Signed application bundles, core (ADR 0013 amendment; to be removed, ADR 0022): `.sursum-app` ZIP with Ed25519-signed manifest, `bundle keygen/create/install/rollback`, DB backup before switching, atomic `current`/`previous` symlinks, `just app-bundle`
- [x] Bundles on the device, server side (ADR 0013 second amendment): trial with a start-counting guard run from the previous release (`ExecStartPre`), automatic rollback incl. the database copy when the schema changed, self-commit after 30 s healthy, upload from the remote (admin, "O programie") with restart into the new release, outcome shown in the remote, `just deploy <host> <version>`; checked with a systemd user unit (crash and hang both roll back)
- Not pursued further (ADR 0022 removes application bundles): `just deploy` against a real Pi. The unit and paths are in the image and the first release ships with it
- [x] Interim hand install for a pilot Pi 5 on Raspberry Pi OS Lite ([deploy/pi-manual](../deploy/pi-manual/README.md)): script, kiosk unit after the rpi-image-gen `webkiosk` example; run on the pilot Pi 5 (2026-11-17). Kiosk unit, policies and cursor generator are shared with the image in `deploy/kiosk/`
- [x] Kiosk cursor: cage (0.3.1 on Raspberry Pi OS) draws its cursor in the middle of the screen even without a mouse and has no option to hide it; and Chromium sets its own cursor after every page reload (from the default/Adwaita theme, e.g. `progress`); the pilot gives the kiosk a private `XCURSOR_PATH` whose `sursum-blank`, `default` and `Adwaita` themes hold one transparent pixel under every cursor name; confirmed on the pilot Pi 5 (no cursor after kiosk start and after two page reloads, 2026-11-17). The `sursum-kiosk` image layer runs the same generator (`deploy/kiosk/blank-cursors.sh`). The black overlay is composited from page load (`will-change: opacity`); the one-frame flash of full-brightness text at the first black did not recur on the Pi afterwards
- [x] `sursum-kiosk` layer (`image/layer/sursum-kiosk.yaml`): Chromium managed policies, short command line, transparent cursor theme, localhost-only network for the kiosk unit (as in `deploy/pi-manual/`; all confirmed on the pilot Pi 5, 2026-11-17: no push-messaging requests after the block, screen and remote work, no cursor or browser UI)
- Moved to S1 (ADR 0009 amendment of 2026-11-19): the WPE-on-DRM spike with a rebuilt 2.54 package and WebKit's own MiniBrowser, without waiting for Debian 14; a Rust launcher only if MiniBrowser is unsuitable
- [ ] PoE evaluation
- [x] Display power, PJLink part (ADR 0012 amendment): codec checked against the JBMIA 1.04 example, background task with polling, state indicator, "on" tap and 1.5 s standby hold in the remote, tested against a fake projector (integration + e2e)
- [x] Display power, CEC part (ADR 0012 amendment of 2026-11-17; both churches' own displays are the open M1 item): `CecDisplay` on `cec_linux` (Image View On + Active Source, Standby, Give Device Power Status), `backend = "cec"` on by default in the image's `/etc/sursum/config.toml`, `/dev/cec0` through `SupplementaryGroups=video` checked on an LG 4K TV (2026-11-17): power state read, on from standby with input switch, standby

**Done when:** a device flashed from the CI image needs no manual steps beyond configuration (PIN,
address, screen profile) and `apt install` on it fails (read-only root); app updates via the browser
work, a deliberately broken bundle rolls back by itself with the matching database, and a
bundle with a bad signature is rejected; power-on shows only black/logo then content (HDMI
capture); 100 power cuts without DB damage, logo + position after each boot; late projector power-on and HDMI replug pass; killed
Chromium/server recovers in < 5 s; Tulce pillar zone calibrated on site and text readable from the
last pew; in both churches the display turns on and to standby from the operator position and the
reported state matches reality in 20 consecutive tries (or a documented "unavailable" reason with
a hardware plan).

## M5 — Pedal and keyboard shortcuts

- [x] Default mapping, ignore key repeat, learn mode, per-device mapping (kept in the browser's `localStorage` — it belongs to the tablet the pedal is paired with and works without pairing; the plan's `devices.prefs_json` is not needed so far); in text fields only PageUp/PageDown act
- [ ] Android: on-screen keyboard with a hardware keyboard paired ⚠ (docs)

**Done when:** mapping tests pass; the organist runs a full service with a pedal only.

## UX — remote redesign (2026-11-17)

Three views (Msza · Zestaw · Ustawienia) with fixed regions, a live strip that keeps black one tap
away everywhere, and plain-text song entry (ADR 0017).

- [x] Layout screenshots at three sizes, operator and admin (`just remote-sizes`)
- [x] Server: gap after an item's last slide (device setting), showing lifts black/logo, `ShowNow` without duplicates, `next_step`, set rename/delete, song ids and numbers in the set view (ADR 0004 amendment)
- [x] Shared UI parts: tokens, one button class, sheet, toast, banner, hold button, PIN field with "show"; notices became toasts, dialogs became sheets, removal toasts offer undo
- [x] Shell and the Msza view: tabs, three layouts, now/slides/set/controls, problem banners, search sheet, live strip, button side, controls disabled without a connection; "#" keypad removed (e2e: `remote_live.rs`)
- [x] Zestaw view: set picker (newest first), new/rename/delete (hold), items always editable (tap opens the song card, never the screen), search that stays open with an "in the set" mark, "new song: <query>" when nothing is found, song card (e2e: `remote_sets.rs`)
- [x] Ustawienia view: screen (power, logo, calibration behind a confirmation), display (gap after an item), this device (button side, vibration, full screen, pedal keys), song library (export; merge and import with reports in a sheet), devices, about; admin items always visible with a lock that asks for the PIN in place
- [x] Editor: plain text (verses by empty lines, "Ref." refrains and repeats), buttons inserting a verse, refrain, bridge or slide break at the cursor, "how to write" help, order under "Zaawansowane", "Zapisz" as the main action, warnings as codes worded in Polish, unsaved-changes guard
- [x] Deleting a set can be undone from the toast (`UndoDeleteSet`)
- [x] ~~Admin mode counts down in the remote~~ (removed with admin mode, ADR 0024)
- [x] Docs: ADR 0017, operator guide rewritten with screenshots (`docs/images/remote/`, from `just remote-sizes`)
- [x] Review of the running remote (2026-11-18, ADR 0017 amendment): sheets leave the controls tappable and focus their field, the live strip fits one row on phones, the next-slide preview fits again after the tablet turns, green only for what is on screen, "Brak logo" without a logo, back gesture between views, editor actions always in view, local pairing dates and device-type names, equal admin rows, contrast of filled green and blue (e2e: `remote_live.rs`)

## M7 — Offline system updates

ADR 0022 (2026-11-19) makes this the only update path; until it is built the text below holds.
Application bundles (`.sursum-app`) update only the server binary with its embedded pages; the
kiosk unit, Chromium policies, cursors, `sursum.service`, Chromium, cage and the kernel are part of
the read-only system. Until system updates exist, changing any of them on an imaged device means
reflashing (with a device backup first). Priority right after the first image works on the Pi.

- [x] Mechanism decided and built (ADR 0019): own updater on the firmware's tryboot and rpi-image-gen's by-slot links; `.sursum-system` (signed ZIP of the raw boot and system images, `just image` writes it), upload from the remote (O programie, admin), root units with the read-only system's verifier write the other slot and reboot with `0 tryboot`, `finish-trial` commits after 30 s of healthy server and kiosk or reboots into the old slot; `panic=10`; unit and integration tests with fake slots; run on the Pi 5 (v0.1.0-36 → v0.1.0-37 committed; the tryboot restart needed `--reboot-argument`, fixed); the remote follows program and system updates in one locked sheet over every view; on the Pi 2026-11-18 a fully automatic update (v39 → v40) committed and a deliberately broken system (kiosk `/bin/false`) rolled back to v40 by itself
- [ ] State shared by both slots (`/var/lib/sursum`) must stay readable by the older slot after a
  rollback (rpi-image-gen warns about this); application releases already handle their own
  database rollback, and ADR 0022 moves that rule into the system trial (S1)
- [x] `just image` also writes the matching `.sursum-system` (and verifies it)
- [ ] The release workflow attaches it once images build in CI

**Done when:** a changed kiosk unit reaches an imaged Pi from the remote without reflashing, data
and pairing survive, and a deliberately broken system update boots back into the old slot by
itself. (All three met on the pilot Pi 5, 2026-11-18; CI-built images remain.)

## D1 — Desktop program (ADR 0020)

One program, `sursum`, with the server inside and the remote in its own window: for preparing
songs at home (export, then "Scal bazę z pliku" in the church) and later for the parish PC.
Work packages and the spike's findings: [plan-desktop-and-ipad.md](plan-desktop-and-ipad.md).

- [x] Spike (branch `spike/desktop-shell`, 2026-11-18; merged into `main` as pull request #1 on 2026-11-19): crate `sursum-desktop`; Linux window on GTK 4 + WebKitGTK 6.0 built and run on Fedora 45 (the remote connects and stays connected); Windows window on `tao` + `wry` cross-compiles with `cargo zigbuild` (9.9 MB, GUI subsystem); clippy clean for both targets; `cargo deny` passes with one exception
- [ ] ⚠ Windows Defender flags the downloaded CI artifact (the ZIP with the unsigned `sursum.exe`) as a virus and deletes it right after the download (2026-11-19). Blocks the guide's download step. To find out: the detection name in Defender's protection history; whether the release's bare `.exe` is flagged too. Ways out: report a false positive to Microsoft (file submission), signing (the ADR 0020 amendment chose unsigned; revisit with an ADR if reporting is not enough)
- [ ] ⚠ First run on Windows (the program starts, 2026-11-19; the rest not yet reported): the window opens, "Eksport bazy" saves a file, "Scal bazę z pliku" opens the file picker, closing the window ends the process; WebView2 present on the parish PC's Windows 10
- [ ] ⚠ Linux by hand: the "save as" dialog of the export, the file picker of the merge, a second start only raises the first window
- [x] The window behaves like a program: the web view's data in `webview/` beside the database, `sursum.log` (previous run kept as `sursum.previous.log`), a Polish message page instead of a silent exit (server failed, another copy running), one copy at a time through `sursum.lock` (Linux also hands a second start to the first window over D-Bus), links outside the server open in the system browser; checked on Fedora 45 (first start, second start, lock held by another process)
- [x] MSVC target on a Windows runner, one `sursum.exe` (ADR 0020 amendment; the WebView2 SDK is BSD-3-Clause, its license is on `/licenses`, which also lists the desktop program's crates and links back to the remote): CI job `desktop-windows` with a non-blocking smoke test (the remote connects, read from `sursum.log`), artifact per commit (4.8 MB zipped), attached to releases; first run on GitHub 2026-11-18 (PR #1): built with MSVC on `windows-2025`, and the smoke test passed there, the remote in the WebView2 window connected to the server; first try on a real Windows (2026-11-19): "VCRUNTIME140_1.dll not found", because MSVC links the C runtime dynamically and the runner has the Visual C++ Redistributable; now linked statically (`.cargo/config.toml`, `+crt-static`) and CI fails if `sursum.exe` imports `vcruntime`/`msvcp`/`WebView2Loader` DLLs; with that it starts on a real Windows without errors (2026-11-19)
- [x] Devcontainer (removed 2026-11-19) (`libgtk-4-dev libwebkitgtk-6.0-dev`, checked on trixie: GTK 4.18.6, WebKitGTK 2.54.0) and CI (Ubuntu 24.04: GTK 4.14.5, WebKitGTK 2.52.6) carry the development packages; `just lint` also runs clippy for the Windows window (`cargo-zigbuild clippy`); `just desktop`, `just desktop-smoke`; all CI jobs green on PR #1 (2026-11-18)
- [x] Library-only view of the remote for a computer without a screen: `SessionInfo.installation` (`screen` | `library`), Pieśni · Ustawienia tabs (search, new song, editor without "Zapisz i pokaż"; export, merge, OpenSong import, backup, about without updates), no screen status or banners (e2e: `remote_library.rs`)
- [x] Local owner access mode (`AccessMode::LocalOwner` replaces the `open_without_pin` flag): admin without a PIN, refused unless the server listens on loopback; `sursum-server --library` (hidden) for the browser tests
- [x] Program icon (`crates/sursum-desktop/assets/sursum.svg`: a lyrics slide in the remote's colours): window icon on Windows, `.exe` icon and name through `winresource` with MSVC (built in CI; how it looks not yet seen), Linux menu entry and icon by the application id (`just desktop-install`)
- [x] Smoke test for the Linux window (`scripts/desktop-smoke.sh`: empty data folder, waits for the remote's connection in `sursum.log`); passes on the Fedora 45 desktop and headless under `dbus-run-session -- xvfb-run -a` (checked in a Fedora container); runs in CI after the build (the runner needs `kernel.apparmor_restrict_unprivileged_userns=0`: WebKitGTK's bubblewrap sandbox aborts the program where unprivileged user namespaces are forbidden ⚠ check on an Ubuntu desktop)
- [x] Operator guide: "Pieśni przygotowane w domu" (download, the SmartScreen warning, write, export, merge in the church, bringing the church's songs home first) — screenshots once the program has run on Windows
- Not planned (ADR 0023): the same program on a parish PC with the screen on the projector, and a macOS build

**Done when:** a parishioner downloads one thing on Windows, starts it without a console or a
browser, writes a song with the preview, exports the library, and the church device merges it.

## P2 — The remote on WebKit (ADR 0021; the iPad itself dropped, ADR 0023)

The remote is tested on WebKit because the desktop program's Linux window is WebKitGTK. The iPad
was prepared without hardware on 2026-11-18 and taken out of scope a day later: nobody has one.

- [x] Remote tests on WebKit, blocking in `just e2e`: WebKitGTK's WebDriver and MiniBrowser under Xvfb in the browsers image (port 9517), `SURSUM_E2E_REMOTE_BROWSER=webkitgtk`; all 30 remote tests pass. Headless WPE was tried first and cannot (view fixed at 800×600; page zoom breaks clicks). Driver differences handled in the harness (`shown_text`, `typed`)
- [x] Audit fixes, kept: safe areas on every side and width (the tablet layouts had dropped the bottom inset), fields at 16px or more, full screen offered only where it exists, buttons clip with `overflow: clip`; found on the way (also in Chromium): the live preview overflowed its row on a 768×1024 tablet upright, and a sheet jumped beside the controls a frame after opening (a tap could close it); `just remote-sizes` covers 4:3 tablet sizes
- [x] ADR 0021: the remote joins the WebKit API contract of ADR 0015
- Removed with ADR 0023: the minimum iPadOS version, the iPad Simulator workflow (written, never run), the real-device checklist and the guide section

## Later

M6 optional HTTPS (device CA) + PWA + wake lock · an x86_64 mini-PC image (`mkosi` +
`systemd-sysupdate`) ·
M8 liturgical calendar/readings (licensing!) · M9 video backgrounds (HEVC on Pi 5) · M10 multiple
screens · M11 internet sync. **MIDI: dropped** (ADR 0002).
