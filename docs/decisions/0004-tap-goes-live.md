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
