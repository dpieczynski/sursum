# 0021 — The remote on WebKit: tested, and bound by the WebKit API contract

Status: Accepted (2026-11-18). Extends ADR 0015 (which covers the screen) to the remote.
Points 1, 4 and 5 (everything about the iPad itself) are superseded by ADR 0023 (2026-11-19);
points 2 and 3 stand, because the desktop program's Linux window is WebKit.

## Context
The plan left the iPad as a "gate left open" (`docs/plan-2026-11.md`): no API without a WebKit
fallback, but no tests. Two things changed. The desktop program shows the remote in WebKitGTK on
Linux (ADR 0020), and parishes may hand an iPad to the organist. Every browser on iPadOS is
WebKit. Nobody on the project has an iPad, so the support has to be prepared without one and
confirmed once on a real device.

Tried on 2026-11-18: headless WPE (the screen's test engine) cannot test the remote, because its
view is fixed at 800×600 and page zoom breaks its click coordinates. WebKitGTK's WebDriver with
MiniBrowser can: a real window whose size WebDriver sets. Its driver differs from chromedriver in
two ways that are not bugs in the remote: element text is "" for anything with `overflow: hidden`,
and "\n" in typed text types nothing.

## Decision
1. **Minimum: iPadOS 16.4** (Safari 16.4). Every WebAssembly feature our build emits works from
   Safari 15 (webassembly.org: bulk memory, non-trapping float-to-int, reference types 15; sign
   extension 14.5 on iOS; multi-value and mutable globals 13.1); the newest CSS the remote uses is
   `overflow: clip` (16); full screen for pages exists on iPad from 16.4.
2. **The remote follows ADR 0015's API contract** where it is cheap, and must not depend on
   anything WebKit lacks without a fallback: `navigator.vibrate` and the Fullscreen API are
   feature-detected through `Reflect` (the web-sys methods throw where they are missing); fields
   are at least 16px (Safari zooms into smaller ones); the page keeps clear of the safe areas.
3. **Tests, blocking**: `just e2e` runs the four remote suites with the remotes in WebKitGTK and
   the screen in Chromium (`SURSUM_E2E_REMOTE_BROWSER=webkitgtk`). The harness hides the two
   driver differences (`shown_text`, `typed`).
4. **Tests, informative**: the `iPad` workflow runs the same suites in Safari on an iPad
   Simulator through `safaridriver` on a macOS runner, by hand and weekly (the repository is
   private; macOS minutes are expensive). It does not block until it has proven stable.
5. **Confirmed only on hardware**: the operator guide calls the iPad supported only after one
   session on a real iPad with the checklist in `docs/testing.md`.

## Consequences
- The browsers image grows by WebKitGTK, its WebDriver, Xvfb and D-Bus; `just browsers` starts a
  third driver (port 9517).
- Things that differ in WebKit's WebDriver are handled in the harness, not by changing the remote
  for the driver's sake; the one exception, buttons with `overflow: clip` instead of `hidden`, is
  the same clipping without a scroll container.
- Screen Wake Lock needs HTTPS (M6); until then the guide asks for Auto-Lock "Never" during the
  service on an iPad.
