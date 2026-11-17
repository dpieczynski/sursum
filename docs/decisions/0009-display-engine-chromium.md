# 0009 — Chromium under cage as the display engine

Status: Accepted (2026-11-15)

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
   first run, pinch and overscroll), and a transparent cursor theme for cage. The pilot hand
   install (`deploy/pi-manual/`) does exactly this; the image's `sursum-kiosk` layer will too.
2. **Target: a small Rust launcher on WPEPlatform's DRM platform**, once the image's base ships
   WPE WebKit ≥ 2.54 (expected with Debian 14 "forky"). Before switching, the measurements from
   the Consequences above run on the Pi 5 (including 4K), and a new ADR records the result and the
   FFI exception.
3. Until then the screen stays engine-portable (ADR 0015): its e2e tests keep running on WPE
   WebKit in CI, so the switch stays cheap.
