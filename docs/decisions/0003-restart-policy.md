# 0003 — Logo after OS reboot, full restore after process restart

Status: Accepted (2026-11-15)

## Context
After a power cut the device must recover. But if someone switches off the power strip after
mass, a literal restore would show last week's slide on the next Sunday. The Pi has no reliable
clock offline (no RTC battery, no internet).

## Decision
`runtime_state` is saved with the OS boot identifier (Linux `/proc/sys/kernel/random/boot_id`;
Windows: system boot time). On start:
- same boot id (only the server or browser restarted) → **full restore**, including the overlay;
- different boot id (power cut, reboot) → restore the set and position with the **logo** overlay.
  One tap on "Show" in the remote reveals the slide.

## Consequences
No dependency on wall-clock time. Platform access goes through a small `Platform` trait so tests
can inject boot ids.

## Amendment (2026-11-25): no logo, black
The logo cover needs a logo. On a device without one, a reboot restores the position under
black instead. Otherwise a logo uploaded later would go on screen by itself, with nobody pressing
LOGO (hard rule 3). The same rule turns `SetOverlay { logo }` without a logo into black, and
removing the logo while it is up leaves black (`Program::settle_cover` in `sursum-core`).
