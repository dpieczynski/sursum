# frontend — local rules

Svelte 5 (runes) + Vite 8 + TypeScript 6 in a pnpm workspace. No SvelteKit.

- `packages/protocol`: generated types (`src/generated/`, never edit by hand) and the WebSocket
  client with reconnect. Regenerate with `just bindings`.
- `packages/render`: the **only** place that draws slides (`Stage`, `SlideView`, `LayeredStage`,
  `fitDeck`). The screen and the remote preview both use it, so the preview matches the screen.
- `packages/i18n`: all user-visible strings (Polish first). No hard-coded UI text in components.
- `apps/screen`: draws only. No UI, no error messages, no cursor, no network resources; black
  background inline in `index.html` `<head>`. Exceptions go to the console and the server log.
- `apps/remote`: operator UI for tablet and PC. Destructive actions need hold-to-confirm plus
  undo; black is always one tap.
- No business logic in the browser: send commands, render snapshots. Font-size fitting is the
  only computation done here (it needs font metrics).
- Components ≲150 lines; logic in plain `.ts` modules; one reactive store fed by the WebSocket.
- Only standard web APIs on the screen (Web Animations, `document.fonts`, `img.decode()`), so the
  display engine can be swapped (ADR 0009).
- SPDX header in every file (`<!-- SPDX-License-Identifier: GPL-3.0-or-later -->` in `.svelte`).
