# apps and shared web crates — local rules

Leptos **0.8** in CSR mode, compiled to WebAssembly with **Trunk** (not cargo-leptos: our server
is our own axum app, not a Leptos SSR server). No Node, npm or TypeScript. Applies to
`apps/screen`, `apps/remote`, `crates/sursum-slide` and `crates/sursum-client`.

- `crates/sursum-protocol`: the shared serde types. Change them there; both the server and the
  apps pick them up. Data only (see root CLAUDE.md, rule 2).
- `crates/sursum-slide`: the **only** place that draws slides (`Stage`, `SlideView`,
  `LayeredStage`, `Masks`, `fit_deck`). The screen and the remote preview both use it, so the
  preview matches the screen. Keep the fit algorithm pure (`fit_font_size` with a measuring
  closure) and the DOM measuring code small.
- `crates/sursum-client`: WebSocket connection (reconnect with backoff, heartbeat, Hello/Welcome,
  reload on a new server build). No UI; apps keep their state in their own signals.
- `apps/screen`: draws only. No UI, no i18n, no error messages, no cursor, no network resources;
  black background inline in `index.html` `<head>`. Panics go to the console
  (`console_error_panic_hook`) and the page reloads itself after 2 s (a panicked module cannot
  send anything to the server; there is no client-to-server log message).
- `apps/remote`: operator UI for tablet and PC. All strings go through the `Msg` enum in
  `apps/remote/src/messages.rs` (compile-time checked; Polish for now). `leptos_i18n` is deferred
  until a second language is needed (ADR 0014 amendment). Destructive actions need hold-to-confirm
  plus undo; black is always one tap.
- `view!` macros contain markup only: build lists and closures in plain functions/variables
  before the macro, otherwise `leptosfmt` and `rustfmt` disagree (`just lint` runs both).
- No business logic in the browser: send commands, render snapshots. Font-size fitting is the
  only computation done here (it needs font metrics).
- Components ≲150 lines; logic outside `view!`; format with `leptosfmt`; `web-sys` instead of
  hand-written `wasm_bindgen` externs; no `unsafe`.
- **Screen API contract (ADR 0015)**: the screen runs on WPE WebKit 2.54 on the device (ADR 0027),
  and CI checks it on WPE 2.54 with headless MiniBrowser. Allowed: WebAssembly, WebSocket, Web Animations, `document.fonts`, `img.decode()`,
  WOFF2 fonts, CSS transforms/opacity/flexbox/grid, `white-space: pre`, CSS variables,
  `ResizeObserver`. Forbidden on the screen: `requestIdleCallback` (use `setTimeout` chunks),
  `scheduler.postTask`, `navigator.userAgentData`, CSS features missing in WebKit 2.54 (unless a
  pure enhancement), any Chromium/kiosk-only API. Feature-detect optional features with a
  fallback. Screen tests use classic WebDriver only (no CDP).
- Test hooks for WebDriver are `data-*` attributes (`data-deck-id`, `data-slide-key`,
  `data-transitions`), not globals.
- Keep `wasm-bindgen` (Cargo.lock) and `wasm-bindgen-cli` (`.github/workflows/`) at the same version.
- The text style (font, colour, size range) is `sursum-slide/src/theme.rs`, not protocol data.
- SPDX header in every file (`<!-- … -->` in `.html`, `/* … */` in `.css`).
