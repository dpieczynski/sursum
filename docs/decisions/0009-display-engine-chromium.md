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
