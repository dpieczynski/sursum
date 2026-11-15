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
The screen uses only standard web APIs; the kiosk command lives in `/data/sursum/kiosk.env`. A
half-day comparison spike with WPE/cog runs in M1 and M4 (boot to first frame, RAM after 8 h,
1080p60 crossfade, white frame at start, HDMI replug). Revisit only with measurements.
