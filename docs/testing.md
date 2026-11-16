# Testing

All tests are Rust; no Node or Python is needed.

| Command | Runs |
|---|---|
| `just test` | unit + integration tests (native), incl. pure frontend logic (`fit_font_size`, backoff, labels) |
| `just e2e` | builds the web apps and an `e2e-hooks` server, then WebDriver tests: all on Chromium, then on WPE WebKit |
| `just lint` | rustfmt, leptosfmt, clippy `-D warnings` (native and `wasm32-unknown-unknown`), SPDX check, cargo-deny |
| `just screenshots [dir]` | PNG screenshots of the screen and the remote (needs `just browsers`) |

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
or export the same `SURSUM_BUILD_ID` yourself. If chromedriver starts answering "Timed out
receiving message from renderer", restart its container (`just browsers-stop && just browsers`).

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
- **Fallback**: if WebDriver + CDP/BiDi cannot cover a case (e.g. inspecting WebSocket frames),
  Playwright for Python may be added for that case only (ADR 0014).
- **On-device (M4)**: 100 power-cut cycles, late projector power-on and HDMI replug, 8-hour soak,
  HDMI capture of boot and failures, app update with deliberate failure and rollback.

## Fixtures

Only public-domain or synthetic texts. Add every new fixture to
[fixtures/LICENSES.md](../fixtures/LICENSES.md). OpenSong fixtures keep their exact bytes
(`.gitattributes` disables EOL conversion for them).

## Testing with a real parish library

Put a copy of the OpenSong folder under `/local/` (git-ignored, never committed), then:

```sh
SURSUM_REAL_LIBRARY=local/OpenSong cargo test -p sursum-formats -- --ignored
```

These tests assert statistics and absence of errors only; they never print or snapshot song text.
