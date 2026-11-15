# 0015 — Screen portability to WPE WebKit (cog), enforced by tests

Status: Accepted (2026-11-16). Complements ADR 0009 (Chromium stays the default engine).

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
