# 0008 — Screen profile with exclusion zones

Status: Accepted (2026-11-15)

## Context
In Tulce part of the projected image falls on a dark pillar where text is unreadable. Moving the
projector is not possible (mounting constraints, heritage conservation).

## Decision
Each device has a `ScreenProfile { safe_area, exclusions: [ExclusionZone { name, rect, mask }] }`.
The server computes the content rectangle with a pure function `content_region` (largest free
axis-aligned rectangle inside the safe area, simple enumeration of candidates) and sends a
`ScreenLayout { content, masks }`. The screen fits text into `content` and paints black masks over
the zones. A calibration panel in the remote (outside services) lets the operator drag the zone
while the screen shows a grid.

## Consequences
Font-size fitting only changes its target rectangle. Two-column layouts around the pillar are
rejected (a verse split by a pillar reads worse). Slanted zones (quadrilaterals with
`clip-path`) can be added later; the model allows it. The profile is per device and is not part
of library export.

## Amendment (2026-11-16): calibration as built
- The test card (`sursum_slide::CalibrationPattern`: lettered/numbered 120 px grid, safe area,
  zones, text rectangle) is the only thing besides content, black and logo that the screen may
  show. It appears only while `ScreenState.calibration` is set by `Command::Calibrate`, is never
  persisted (a restart always ends it) and is opened in the remote with a hold.
- The remote panel keeps no copy of the profile: each drag or field edit sends
  `Command::SetScreenProfile`; the server validates it (`InvalidProfile` if no room for text is
  left), stores it in `settings.screen_profile` and broadcasts the new layout. The tablet draws
  the same `CalibrationPattern` from that snapshot, so tablet and screen cannot disagree.
- A profile saved from a remote wins over `config.toml` (logged at startup). There is no admin
  role yet (M3 roles item); until then any paired remote may calibrate.
