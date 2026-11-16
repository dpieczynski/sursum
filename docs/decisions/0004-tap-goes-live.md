# 0004 — Tapping a slide in the remote goes live immediately

Status: Accepted (2026-11-15)

## Context
The organist operates with one hand; two-step "cue then take" doubles the actions.

## Decision
Tapping a slide, section chip or set item shows it on screen immediately (like OpenSong). A wrong
tap is fixed by another tap.

## Consequences
The program model keeps an unused `cue` field so a per-device "preview first" mode can be added
later without protocol changes. Destructive operations (deleting items) still require hold to
confirm plus undo; black is always one tap.

## Amendment (2026-11-17): showing lifts black; a gap between items
- Tapping a set item or a slide, and "Pokaż teraz", lift black or the logo: the operator chose
  what the congregation should see, and before this a tap after a reboot (logo) or after black
  changed the slide invisibly. Relative steps (Next, Prev, next/previous item) keep black or the
  logo, so a slide can still be prepared unseen.
- "Next" on an item's last slide first shows an empty slide (the gap), then the next item, so the
  next song's words do not appear before it starts. A device setting (`SetEndOfItem`) turns it
  off. The gap is content, not an overlay; black and logo stay the operator's own tools.
- "Pokaż teraz" for a song that is already in the active set goes to that item instead of adding
  it again.

