# 0015 — Screen portability to WPE WebKit (cog), enforced by tests

Status: Accepted (2026-11-16). Complements ADR 0009 (Chromium stays the default engine). Since ADR
0027 (2026-11-25) WPE WebKit is the shipped engine, so this contract now describes the device; the
tests still start WPE 2.48 through cog.

## Context
ADR 0009 keeps Chromium under cage but leaves WPE WebKit + cog as the most likely alternative
(lighter, can draw directly to KMS). "Use only standard web APIs" is a good intention but not a
guarantee; without tests the screen would drift into Chromium-only behavior.

## Decision
1. **Screen API contract** (`apps/screen`, `sursum-slide`, `sursum-client`):
   - allowed: WebAssembly, WebSocket, Web Animations API (`Element.animate`, `Animation.finish()`,
     `finished`), `document.fonts`, `HTMLImageElement.decode()`, WOFF2 `@font-face`, CSS
     transforms/opacity/flexbox/grid, `white-space: pre`/`pre-wrap`, CSS variables,
     `ResizeObserver`;
   - forbidden on the screen: `requestIdleCallback` (disabled by default in WebKit; use
     `setTimeout` chunks), `scheduler.postTask`, `navigator.userAgentData`, CSS features missing
     in WebKit 2.48 unless used as progressive enhancement, any kiosk/Chromium-only API;
   - optional features are feature-detected with a fallback; at startup the screen sends a
     compatibility report (engine, missing features) to the server log, never to the display.
2. **Tests on real WPE**: the devcontainer and CI install `cog` and `wpewebkit-webdriver`
   (`WPEWebDriver`, WPE WebKit 2.48.3 in Debian Trixie). The screen subset of the WebDriver suite
   runs **blocking** on WPE (cog `--automation --platform=headless --bg-color=black`): black first
   frame, crossfade and no crossfade on reconnect, last slide kept when the server stops, one font
   size per song, exclusion zones, set edits not affecting the screen. Screen tests use classic
   WebDriver only (no CDP). `wasm-bindgen-test` for `sursum-slide` also runs on WPE. A
   cross-engine test checks that WPE (screen) and Chromium (remote) fit the same song to the same
   font size (±1 px) and line count; if that ever fails, enable `FitReport` (remotes adopt the
   screen's size).
3. **Engine switch in the image**: the `sursum-kiosk` layer takes `kiosk.engine = chromium | cog`.
   The cog variant runs `cog --platform=drm` directly on KMS (no cage) with mandatory
   `--bg-color=black` (cog's default background is white) and `--webprocess-failure=exit` (the
   default shows an error page; exiting lets systemd restart the kiosk). The screen watchdog is
   engine-independent. CI builds the cog image variant weekly.
4. **Switching engines** requires the hardware comparison from the M1/M4 spikes (boot to first
   frame, RAM after 8 h, 1080p60 crossfade, white frame at boot, HDMI replug, renderer crash) and a
   new ADR.

## Consequences
- An extra CI job (WPE screen tests) and two Debian packages in the devcontainer.
- Some conveniences are off-limits on the screen (e.g. `requestIdleCallback`); the remote is not
  bound by this contract (it targets Blink) but should follow it where cheap.
- The exact WPEWebDriver capability for browser options is verified in the M0 spike.

## Amendment (2026-11-17): no cog image variant
Point 3 is withdrawn: cog is legacy (no stable releases beyond 0.18.x, ADR 0009 amendment), so the
image gets no `kiosk.engine = cog` variant and CI builds none. The planned WPE engine is a small
Rust launcher on WPEPlatform's DRM platform once the base ships WPE WebKit ≥ 2.54. Points 1 and 2
stay: the screen keeps its API contract and its blocking WPE tests. Those tests still start WPE
through cog (`--automation --platform=headless`) because WPEWebDriver needs a launcher and trixie
has nothing newer; they move to a WPEPlatform launcher together with the engine.

## Amendment (2026-11-19): what the WPE tests are worth until the spike
The blocking WPE tests run WPE WebKit 2.48.3 through cog: an engine version and a launcher that
will never ship on a device. They guard the screen's API contract, nothing more. The spike of
ADR 0009's amendment of the same day decides their future: if WPE on DRM becomes the engine, the
tests move to the shipped combination (Debian forky and sid carry WPE WebKit 2.54 with
`wpewebkit-webdriver`; WPEWebDriver starts `MiniBrowser --automation` when given its path as
`binary` ⚠ not yet tried), and cog leaves the browsers image; if the spike fails, the WPE tests
and cog are removed and the contract is checked by the WebKitGTK tests of ADR 0021 alone.

The compatibility report of point 1 ("the screen sends a compatibility report to the server
log") was never built; its message, `ClientMsg::Log`, had no sender and was removed on
2026-11-19. Problems on the screen show in the kiosk's journal.
