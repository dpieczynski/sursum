# 0002 — No MIDI support

Status: Accepted (2026-11-15)

## Context
MIDI from the organ was considered as a "next slide" trigger.

## Decision
The parish does not want MIDI. Buttons on the remote and a pedal/presenter clicker paired with
the remote device are enough.

## Consequences
No `midir`/ALSA dependency, which keeps cross-compilation simple. The roadmap has no MIDI stage.
