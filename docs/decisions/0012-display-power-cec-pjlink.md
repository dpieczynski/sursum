# 0012 — Display power control via HDMI-CEC, with PJLink fallback

Status: Accepted (2026-11-16); per-church backend to be confirmed by the M1 hardware spike

## Context
The infrared remotes of the projector (Tulce) and the 86" monitor (Gowarzewo) do not reach the
operator position. Operators need to switch the display on and to standby from the sursum remote.

## Decision
- The remote shows the display power state and offers **"Turn on"** (one tap) and **"Standby"**
  (hold 1.5 s, extra warning when the program shows content rather than black/logo).
- The server talks to the display through a `DisplayPower` trait with backends:
  - **HDMI-CEC** (default on the Pi) through the Linux kernel CEC API (`/dev/cec0` = HDMI0) using
    the pure-Rust crate `cec_linux` (0.2.2): `Image View On` + `Active Source` to power on,
    `Standby` to the TV, `Give Device Power Status` for the state. No libcec / C dependency.
  - **PJLink class 1** (TCP 4352) for projectors with a LAN port: a small own implementation,
    since the `pjlink` crate (0.2.0) has not been updated since 2019. Works on Windows too.
  - **None**; later possibly **RS-232** with vendor commands from config.
- Commands run in a separate task; the state actor never waits for the display. The state is
  polled every 30 s and right after each command.
- The display is not switched on automatically at boot (`power_on_at_boot = false` by default).

## Consequences
- The Pi must be connected to the display's HDMI input directly (ADR 0001), on the Pi's HDMI0
  port; HDMI extenders often drop CEC.
- With the forced HDMI mode and a sleeping display without EDID, the Pi may not know its physical
  address; a configurable physical address (or the fallback EDID file) covers `Active Source`.
- Projectors often lack CEC: the M1 spike checks CEC (`cec-ctl` from `v4l-utils`), PJLink and
  RS-232 on both displays and records the chosen backend here.
- Protocol: command `DisplayPower { action }`, `RemoteState.displayPower`, error codes
  `DisplayUnreachable`, `DisplayBusy`, `DisplayAuthFailed`.

## Amendment (2026-11-16): PJLink as built
- PJLink class 1 is implemented from the JBMIA specification 1.04 (2013-12-10): a pure codec in
  `sursum_core::display_power` (greeting, MD5 digest of seed‖password — checked against the
  specification's own example — power commands and replies) and a background task in
  `sursum-server::display_power` that opens one connection per exchange, polls every 30 s
  (every 5 s while warming up or cooling down) and reports only changes to the state actor.
- Configuration: `[display_power] backend = "pjlink"`, `host = "address[:port]"`, optional
  `password` in `config.toml` (device only).
- The backends implement the `DisplayPower` trait (`NoDisplay`, `PjLinkDisplay`; a
  `FakeDisplay` in unit tests drives the polling task). Its methods return `impl Future + Send`
  and the task is generic over the backend, so there are no trait objects and no `async-trait`
  crate. Integration tests also run the real PJLink client against a fake projector over TCP.
  CEC becomes a third implementation after the hardware spike (M4); `cec_linux` and the CEC
  frames are still unverified ⚠.
- Standby uses a 1.5 s hold in the remote and warns while a slide is on screen; "on" is
  disabled while a projector cools down.
