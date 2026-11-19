# 0023 — Platform scope: no iPad, no macOS, no Windows parish PC

Status: Accepted (2026-11-19). Supersedes points 1, 4 and 5 of ADR 0021 and closes the open
question of ADR 0020 about the program on a parish PC.

## Context
Three targets were being prepared without anyone needing them: the remote on an iPad (nobody on
the project or in the parish has one; the iPad Simulator workflow was written but never run), the
desktop program on macOS (not started), and sursum on the Windows PC next to a projector (the
P1 pilot script was written and never run; the device image has run on the pilot Pi 5 since
2026-11-17). Each kept code, CI jobs or roadmap items alive.

An audit on 2026-11-19 also asked whether everything could run on one engine. It cannot: there
is no WebKit that can be embedded on Windows (the upstream Windows port has no release or SDK),
Chrome on the Android tablets is Blink, and bundling Chromium (CEF) into the desktop program
costs 135 to 315 MB per platform. `wry` on Linux still needs GTK 3 with WebKitGTK 4.1 (its GTK 4
port is an open community pull request), and the maintainer's Fedora 45 KDE image installs only
WebKitGTK 6.0, so one `wry` module for both desktop platforms is not possible yet. The remote
therefore runs on two engines whatever we choose, and the desktop program uses each system's own
web view.

## Decision
1. **The remote**: Chrome on Android tablets, Chromium/Chrome/Edge on Linux and Windows PCs.
   No iPad: no minimum iPadOS version, no iPad Simulator workflow, no real-device checklist, no
   guide section.
2. **The desktop program**: Windows (the product: WebView2 through `tao` + `wry`) and Linux
   (best effort: `gtk4` + `webkit6`), as in ADR 0020. No macOS build.
3. **The screen**: the Raspberry Pi 5 device image only. No Windows pilot on a parish PC and no
   projector mode in the desktop program; plan B during the first services is the existing
   OpenSong PC, as the operator guide says.
4. **What stays of ADR 0021**: the remote keeps to the WebKit API contract and its four suites
   stay blocking on WebKitGTK, because the Linux window of the desktop program is WebKit. The
   small things done for Safari (safe-area insets, 16 px fields, feature detection for
   `navigator.vibrate` and full screen, `overflow: clip`) stay; they cost nothing and are right
   on any engine.

## Consequences
- Removed on 2026-11-19: `.github/workflows/ipad.yml`, the `safari-ipad` engine in the e2e
  harness, `deploy/windows/start-pilot.ps1`, `just cross-win`, and the `sursum-server.exe` of
  the CI and release workflows. The server still compiles for Windows inside the desktop
  program.
- Roadmap: P1 is closed without a run, P2 keeps only its WebKitGTK tests, and the "later" item
  about the parish PC is gone.
- Any of the three comes back only with a new ADR and someone who needs it. The remote is a web
  page, so an iPad may well work; nobody checks, and the guide does not promise it.
- When `wry` and `tao` release on GTK 4 with WebKitGTK 6.0, one window module for Windows and
  Linux becomes possible; until then the two modules (about 190 lines) are the smallest
  arrangement.
