# 0027 — WPE WebKit on DRM replaces Chromium under cage

Status: Accepted (2026-11-25). Supersedes the engine choice of ADR 0009 and closes its WPE-on-DRM
spike (amendment of 2026-11-19); ADR 0015's API contract stays and now describes the shipped
engine.

## Context
The spike of ADR 0009's amendment ran on the pilot Pi 5 on 2026-11-24/25: an image of v0.3.0 with
WebKit's own MiniBrowser from our trixie rebuild of WPE WebKit 2.54.3 on the DRM platform, no
compositor, installed as a system update next to the v0.3.0 release with Chromium 156 under cage.
Debian's package does not declare everything MiniBrowser and the web process need at run time;
two trials rolled back by themselves until the layer named `libwpebackend-fdo-1.0-1`,
`bubblewrap`, `xdg-dbus-proxy` and `libgles2`. Then:

| | Chromium under cage | WPE on DRM |
|---|---|---|
| System image | 549 MB | 429 MB |
| Kiosk memory (PSS, all processes) | 524 MB | 241 MB |
| Screen connected after the kernel started | ≈ 13 s | ≈ 10.5 s |
| Seen at boot | black → gray frame → logo | black → logo |
| Display mode | cage first set the TV's 4K30, `kiosk-session` then 1080p60 | the kernel's forced 1080p60, never changed |
| Crossfades, logo, cursor, display off/on, HDMI replug | fine | fine |
| A killed web process | (not measured) | logo or black until the screen guard restarted the kiosk, 15.5 s |

The one thing Chromium did that WPE cannot: Chromium's `start.html` (a local page that asks the
server and then opens the screen) kept the screen black while the server was down. WebKit does
not let a `file://` page reach the server, even with universal file access allowed.

## Decision
The screen is WPE WebKit's MiniBrowser on the DRM platform (`--fullscreen --bg-color=black
http://localhost/screen/`, `WPE_PLATFORM=drm`), from Debian's sid source package rebuilt unchanged
for trixie arm64 (`.github/workflows/wpewebkit.yml`). The kiosk unit starts it only once the
server answers `/healthz` (`deploy/kiosk/wait-for-server`); until then nothing holds the display
and it stays black, at boot and at every kiosk restart. Checked on the Pi with a crash-looping
server: black throughout, the screen back 0.5 s after the server.

Removed: cage, seatd, `wlr-randr`, `kiosk-session`, the transparent cursor theme, Chromium's
managed policies, Chromium's kiosk unit and `start.html`.

## Consequences
- The display mode comes only from the kernel command line (`video=HDMI-A-1:1920x1080@60D`, ADR
  0013); `SURSUM_SCREEN_MODE` in `kiosk.env` is gone. Another mode for an installation means
  another command line. `KIOSK_APP` in `kiosk.env` still overrides the browser command.
- We carry the engine package ourselves: trixie stays at 2.48 and Debian never rebases it, so
  security fixes arrive only when we rebuild a newer sid version. The kiosk loads only our page
  from localhost and its unit can reach nothing else, which keeps that risk small. The packages
  live as a workflow artifact, which expires after 90 days: rebuild before then (or keep them
  somewhere durable; roadmap).
- Every image build needs the packages (`just wpe-debs`, and a step in image.yml and
  release.yml). A release takes only a build on main started by a push or by hand, never a pull
  request's, because the release job signs what it installs.
- The wait for the server runs inside the kiosk's `ExecStart` and then becomes the browser. In
  `ExecStartPre` the unit would stay "activating" while the server is down, and the system
  update's trial, then ordered after the kiosk, would never start its clock or roll back; the trial
  is now ordered after the server only.
- A dead web process is not restarted by MiniBrowser; the screen guard's grace (15 s) decides how
  long the screen shows black or the logo. Shortening it is a separate question.
- The WebDriver screen tests still run WPE 2.48 through cog (ADR 0015), not 2.54 with
  MiniBrowser; moving them is follow-up work. Chromium stays in the tests for the remote.
- Memory over time, measured after the decision on the same Pi (2026-11-25; never measured for
  Chromium): an idle screen does not grow. 2,000 slide changes with black and logo rounds took
  the kiosk from 228 MB to 269 MB, which then stayed flat for 53 minutes; even if that were all a
  leak it is about 3 MB per service against 3.6 GB free, and every reboot and system update
  restarts the kiosk. Details in the roadmap.
