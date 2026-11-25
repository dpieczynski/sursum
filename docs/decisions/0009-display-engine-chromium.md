# 0009 — Chromium under cage as the display engine

Status: Superseded by ADR 0027 (2026-11-25): WPE WebKit on DRM replaced Chromium under cage after
the spike of the amendment of 2026-11-19. Accepted 2026-11-15.

## Context
Candidates on Debian Trixie: Chromium (150), WPE WebKit 2.48.3 + cog 0.18.4, Firefox ESR 140, or
a native Rust renderer.

## Decision
Chromium in kiosk mode under cage. The decisive reason: all target remotes use Blink (Chrome on
Android, Chromium on Linux, Edge/Chrome on Windows), so the preview matches the screen without
extra work. Raspberry Pi maintains and accelerates Chromium.

Rejected: WPE/cog (lighter and can draw directly to DRM, but a different engine than the
remotes); Firefox (a different engine *and* similarly heavy, so it combines the drawbacks of both
alternatives; the Pi platform optimizes Chromium first; ESR lags features); native renderer
(would break "nothing implemented twice", the preview would need the same renderer in WASM).

## Consequences
The screen uses only standard web APIs; the kiosk command lives in `/var/lib/sursum/kiosk.env`. A
half-day comparison spike with WPE/cog runs in M1 and M4 (boot to first frame, RAM after 8 h,
1080p60 crossfade, white frame at start, HDMI replug). Revisit only with measurements.

## Observation (2026-11-16): browser UI over the screen
First run on a real TV (x86 PC, Chromium 157 flatpak in `--kiosk`): Chromium offered to translate
the Polish page, a bubble over the screen, although the kiosk command had
`--disable-features=Translate`. Command-line flags are not a stable contract. Mitigations now: both
pages opt out (`<html translate="no">`, `<meta name="google" content="notranslate">`, checked by
the first-frame e2e test); in the device image, Chromium **managed policies** (e.g.
`TranslateEnabled: false`) rather than flags. The spike's checklist adds: "any browser UI ever
appears over the page" — a point in favour of cog, which has no browser UI at all.

## Amendment (2026-11-17): Chromium for now, a WPEPlatform launcher later; cog is out
The pilot Pi 5 (Raspberry Pi OS, cage 0.3.1, Chromium 154) needed one workaround after another:
translation, cage's cursor in the middle of the screen (also after every page reload, when
Chromium sets its own), and background requests to Google services. The maintainer asked whether
WPE would be simpler. Findings:

- **cog is legacy.** The WPE FAQ: "Cog is the launcher built on the legacy API (pre-2.54). It is
  kept for existing deployments, but new projects do not need it"; the 2.52 release notes: no
  stable releases beyond 0.18.x. cog is therefore rejected, also as an interim engine.
- **The WPE way forward is WPEPlatform** (default since WPE WebKit 2.54): a launcher of a few
  dozen lines, with a DRM/KMS platform that draws without any compositor (no cage, no cursor, no
  browser UI).
- **Rust can drive it**, but nothing ready exists: no maintained crates for WPE WebKit 2.0 /
  WPEPlatform (webkit6-rs covers WebKitGTK), and Debian builds WPE with
  `-DENABLE_INTROSPECTION=OFF`, so bindings cannot be generated from distribution packages. The
  launcher would be a small hand-written FFI module (about ten C functions) plus the `glib` crate
  for the main loop and signals; that module would need `unsafe`, an exception to record then.
- **The blocker is the version, not the language.** Debian trixie (our base) has WPE WebKit
  2.48.3, and Debian does not move WPE to new upstream versions within a stable release
  (bookworm stayed at 2.38.6); 2.54.2 is in forky/sid. Building WebKit ourselves in the image
  would mean hours of arm64 builds and our own security updates; Raspberry Pi keeps Chromium
  current.

Decision:
1. **Chromium under cage stays the engine for now.** Its workarounds live in one documented
   place: Chromium **managed policies** for everything a policy covers (translation, component
   updates, metrics, Safe Browsing, search suggestions, sign-in, sync, passwords, spellcheck,
   notifications, background mode, developer tools, printing; names and values checked against
   the policy definitions in Chromium's source), a short command line for what no policy covers
   (`--disable-background-networking` for the remaining background requests, crash bubbles,
   first run, pinch and overscroll), and a transparent cursor theme for cage. Network access is
   not left to the browser: the kiosk unit allows only localhost (`IPAddressAllow=localhost`,
   `IPAddressDeny=any`), because Debian's Chromium carries Google API keys and kept registering
   for push messaging on the Pi despite the flag. The pilot hand
   install (`deploy/pi-manual/`) does exactly this; the image's `sursum-kiosk` layer will too.
2. **Target: a small Rust launcher on WPEPlatform's DRM platform**, once the image's base ships
   WPE WebKit ≥ 2.54 (expected with Debian 14 "forky"). Before switching, the measurements from
   the Consequences above run on the Pi 5 (including 4K), and a new ADR records the result and the
   FFI exception.
3. Until then the screen stays engine-portable (ADR 0015): its e2e tests keep running on WPE
   WebKit in CI, so the switch stays cheap.

## Amendment (2026-11-19): a WPE-on-DRM spike now, with a rebuilt package and the shipped launcher
The maintainer remains unconvinced by Chromium and asked whether a current WebKit could simply be
bundled. Checked on 2026-11-19 against the Debian and Raspberry Pi archives, the WebKit source at
`wpewebkit-2.54.3` and the upstream sites:

- **The decisive reason of this ADR no longer holds.** "All remotes use Blink" stopped being true
  with the desktop program's Linux window (WebKitGTK, ADR 0020); the remote is tested on two
  engines anyway (ADR 0021), and a cross-engine test keeps the fitted size within 1 px.
- **Chromium's cost in the repository is small and paid**: 17 lines of policies and a dozen
  flags, all confirmed on the pilot Pi. The larger part of `deploy/kiosk/` (the transparent
  cursor theme, `kiosk-session` with `wlr-randr`, seatd) is there for cage, not for Chromium.
  cage plus Chromium is also Raspberry Pi's own kiosk recipe (rpi-image-gen's `webkiosk`), with
  their patched builds of both.
- **WebKitGTK 6.0 under cage** would be free to install (2.54.0 in trixie-security, which
  Debian rebases to new upstream series) but keeps cage and everything written for it, removes
  only the policies, needs a GTK launcher cross-built for arm64, and has an open rendering bug
  on the Pi 5 with the same engine version (WebKit bug 329376, reported for the GTK 3 API; the
  workaround costs a CPU core). Rejected.
- **WPE WebKit on DRM** is the only candidate that removes something: no compositor, so no cage,
  seatd, `wlr-randr`, `kiosk-session`, cursor theme or policies (about 150 lines), and a smaller
  system. Since 2.54 WPEPlatform with its DRM platform is stable and built by default.
- **Getting 2.54 onto trixie**: trixie stays at 2.48.3 (not security-supported; Debian never
  rebases it), there is no backport, no upstream or Igalia package repository and no arm64
  bundle, and forky's package cannot be installed on trixie (glibc 2.43, ICU 78 and more). But
  Debian's own source package (2.54.3 in sid) can be rebuilt against trixie: about one hour on
  Debian's arm64 build machines, about 2.5 hours reported on a 4-core arm64 CI runner, a day on
  the Pi itself. Security support matters little here: the kiosk loads only our own page from
  localhost and its unit can reach nothing else.
- **No launcher of ours is needed to start.** Debian's library package ships WebKit's
  `MiniBrowser` (`/usr/lib/<triplet>/wpe-webkit-2.0/MiniBrowser`), which uses WPEPlatform by
  default and takes `--fullscreen` and `--bg-color` (its default background is white). The
  hand-written FFI launcher with its `unsafe` exception (point 2 of the amendment of
  2026-11-17) is written only if MiniBrowser turns out unsuitable.

Decision: **do not wait for Debian 14. Run the spike now**; Chromium stays the shipped engine
until its measurements are in, and the switch still needs its own ADR.

1. Rebuild sid's `wpewebkit` source package for trixie arm64 in CI (an arm64 runner, available
   once the repository is public) and keep the packages as a build artifact the image installs.
2. A spike image whose kiosk unit runs `MiniBrowser --fullscreen --bg-color=black
   http://localhost/screen/` with `WPE_PLATFORM=drm`, without cage; delivered to the pilot Pi as
   a system update, so a failure rolls back by itself (ADR 0019).
3. ⚠ To find out on the Pi 5, none of it reported anywhere yet: that the DRM platform starts at
   all without a logind session and picks the right card; that it keeps the kernel's forced
   1080p60 (it takes the current CRTC mode and has no option to set one) also after the TV is
   switched on late or the cable is replugged; that no cursor shows (it draws one only when the
   page asks; the screen should set `cursor: none`); what happens when the web process dies
   (the screen guard restarts the unit); the measurements from the Consequences above (boot to
   first frame, RAM after 8 h, 1080p60 crossfade, white frame at start); the size of the system
   image.
4. If the spike fails, Chromium under cage stays for good, and the WPE tests and the launcher
   plan are dropped with an ADR.
