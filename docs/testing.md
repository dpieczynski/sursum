# Testing

All tests are Rust; no Node or Python is needed.

| Command | Runs |
|---|---|
| `just test` | unit + integration tests (native), incl. pure frontend logic (`fit_font_size`, backoff, labels) |
| `just test-wasm` | `wasm-bindgen-test` in Chromium (via the chromedriver container): DOM measuring of `fit_deck` — chosen size fits and is the largest, longer lines/narrower areas shrink, one size per deck, independent of stage scaling |
| `just e2e` | builds the web apps and an `e2e-hooks` server, then WebDriver tests: all on Chromium, then on WPE WebKit, then the remote suites with the remotes on WebKitGTK (ADR 0021) |
| `just lint` | rustfmt, leptosfmt, clippy `-D warnings` (native, `wasm32-unknown-unknown`, and the desktop program's Windows window), SPDX check, cargo-deny |
| `just desktop-smoke` | starts the desktop program with an empty data folder until the remote in its window connects (needs a display; CI uses `dbus-run-session -- xvfb-run -a`) |
| `just screenshots [dir]` | PNG screenshots of the screen and the remote (needs `just browsers`) |
| `just remote-sizes [dir]` | PNG screenshots of the remote at 360×800, 800×1280, 1280×800 and iPad sizes (768×1024, 1024×768, 820×1180), operator and admin (needs `just browsers`) |

## WebDriver servers

The browsers run in a podman image with host networking, bound to localhost:

```sh
just browsers-image   # once: Chromium + chromedriver, cog + WPEWebDriver, Mesa
just browsers         # chromedriver on :9515, WPEWebDriver on :9516
just e2e
just browsers-stop
```

Environment for `cargo test -p sursum-e2e -- --ignored`: `SURSUM_E2E_BROWSER=chromium|wpe`,
`SURSUM_SERVER_BIN` (default `target/debug/sursum-server`), `SURSUM_WEBDRIVER_URL`. Each test
starts its own server on a random port with a temporary data directory; the server's stderr is
kept in that directory and printed if it fails to start. WPEWebDriver allows one session at a
time, so tests always close their session (`with_browser`) and run with `--test-threads=1`.

**Build ids must match.** `just` exports `SURSUM_BUILD_ID` (`git describe --dirty`) for the web
apps; a server built with plain `cargo build` has a different id, and the pages then reload in a
loop (by design: a client never talks to a server of another build). Build both through `just`,
or export the same `SURSUM_BUILD_ID` yourself. The containers run with `--init`: without an init
process the finished browser processes stay as zombies and after a few hundred sessions chromedriver
answers "Timed out receiving message from renderer" (`just browsers-stop && just browsers` if you
started them another way).

## Layers

- **Rust unit tests** (`sursum-protocol`, `sursum-core`, `sursum-formats`): OpenSong parser edge
  cases, slide compiler, program state machine (bounds, ghost position, live edits, overlay,
  `Stale` rule), Polish normalization and ranking, `content_region`, library merge, CEC/PJLink
  encoding. `proptest` for invariants (set operations, navigation bounds, layout never intersects
  a zone, `fit_font_size` monotonicity). `insta` JSON snapshots of protocol messages.
- **Protocol integration tests** (`sursum-server/tests`, tokio-tungstenite): server on port 0 with
  a `tempfile` data dir; hello/snapshot, commands, two remotes, rejected `Origin`, missing cookie,
  PIN rate limit, restart with the same/different `boot_id` (injected `Platform`), abort during a
  write, app-bundle signature and rollback.
- **WASM tests** (`wasm-bindgen-test` with `chromium-driver`): DOM measuring in `fit_deck` (same
  result in a scaled and an unscaled stage, no line wrapping), waiting for fonts/images before a
  crossfade, reconnect logic in `sursum-client` against a fake WS server.
- **WebDriver end-to-end** (`e2e/`, crate `sursum-e2e`, thirtyfour 0.38 with `cdp`/`bidi`): the
  harness starts the real server binary (built with the test-only `e2e-hooks` feature) and opens
  separate browser sessions for the screen and each remote. Chromium is blocking; Firefox
  (geckodriver) is non-blocking. Key tests: black first frame (server delays the WASM); editing
  the set never changes the screen (`data-deck-id`, `data-slide-key`, `data-transitions`,
  screenshot compared with the `image` crate); last slide stays when the harness stops the server
  and reconnect has no transition; remote offline via CDP network emulation; one font size per
  song; preview matches screen; black in one tap; hold-to-delete + undo; pedal keys via WebDriver
  key actions; two remotes in sync; text never enters an exclusion zone; display power with the
  fake backend.
- **Second screen engine (ADR 0015)**: the screen subset of the WebDriver suite also runs
  **blocking** on WPE WebKit via `WPEWebDriver` + `cog --automation --platform=headless`
  (black first frame, crossfade/no crossfade on reconnect, last slide kept, one font size per
  song, exclusion zones, set edits not affecting the screen), using classic WebDriver only (no
  CDP). `wasm-bindgen-test` for `sursum-slide` runs on WPE too. A cross-engine test compares the
  fitted font size and line count between WPE (screen) and Chromium (remote).
- **The remote on WebKit (ADR 0021)**: the remote suites (`remote`, `remote_live`,
  `remote_sets`, `remote_library`) run **blocking** with `SURSUM_E2E_REMOTE_BROWSER=webkitgtk`:
  WebKitGTK's `WebKitWebDriver` and MiniBrowser under Xvfb (port 9517), the screen on Chromium.
  Headless WPE cannot do this (its view is fixed at 800×600). WebKit's driver returns "" as the
  text of anything with `overflow: hidden` and types nothing for "\n": read text with
  `shown_text` and type lines with `typed`. The `iPad` workflow (by hand, weekly) runs the same
  suites in Safari on an iPad Simulator (`safari-ipad`, macOS runner), not blocking yet.
- **Fallback**: if WebDriver + CDP/BiDi cannot cover a case (e.g. inspecting WebSocket frames),
  Playwright for Python may be added for that case only (ADR 0014).
- **On-device (M4)**: 100 power-cut cycles, late projector power-on and HDMI replug, 8-hour soak,
  HDMI capture of boot and failures, app update with deliberate failure and rollback.

## Fixtures

Only public-domain or synthetic texts. Add every new fixture to
[fixtures/LICENSES.md](../fixtures/LICENSES.md). OpenSong fixtures keep their exact bytes
(`.gitattributes` disables EOL conversion for them).

## iPad on a real device (once, before the guide calls it supported)

Prepared without hardware (ADR 0021); one session on a real iPad (iPadOS 16.4 or later; a
borrowed one is enough, an iPhone covers everything but the layout) confirms it. Synthetic or
public-domain songs only. Note the iPad model and iPadOS version, then check:

1. Pairing through `http://sursum.local/` on the church network (and by IP address if `.local`
   fails), PIN entry with the on-screen keyboard.
2. The Msza view upright and lying: nothing overlaps, nothing sits under the home indicator or
   the rounded corners; turning the iPad keeps the layout.
3. Taps: Dalej, Wstecz, a tapped set item goes live; black is one tap away with a sheet open.
4. Hold buttons (Uśpij ekran, deleting a set): a long press does not open Safari's menu.
5. The search and the editor: the page does not zoom when a field gets focus; the live strip
   stays usable with the on-screen keyboard open.
6. Lock the iPad for a minute, unlock: the remote reconnects by itself; switch to another app
   and back: the same.
7. A whole set by touch, then by a Bluetooth pedal (page keys).
8. Ustawienia → Eksport bazy saves to Files; Scal bazę z pliku picks it from Files.
9. Full screen (Ustawienia → To urządzenie) on, then off.
10. Add to Home Screen, open from there: everything above still works.

Write the result (model, iPadOS version, what failed) into the roadmap's P2 section.

## Testing with a real parish library

Put a copy of the OpenSong folder under `/local/` (git-ignored, never committed), then:

```sh
SURSUM_REAL_LIBRARY=local/OpenSong cargo test -p sursum-formats -- --ignored
```

These tests assert statistics and absence of errors only; they never print or snapshot song text.
