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
