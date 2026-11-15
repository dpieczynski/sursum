# 0001 — Raspberry Pi 5 next to the screen, Ethernet, optional PoE

Status: Accepted (2026-11-15)

## Context
Today a Windows PC at the operator position feeds the projector (Tulce) / 86" monitor (Gowarzewo)
through an HDMI-over-Ethernet extender. Commercial systems put a mini-PC at the screen and control
it over the network.

## Decision
The Pi 5 sits next to the projector/monitor, connected with a short HDMI cable and to the parish
network by **Ethernet**. Remotes (Android tablet, Linux/Windows PC) connect over the LAN. The
existing extender cable may be reused as the Ethernet link. PoE is optional: there is no official
Pi 5 PoE HAT, but third-party 802.3af/at HATs with 5 V/5 A output exist (e.g. Waveshare PoE HAT
(H)/(G)/(J), 52Pi P30) and need a PoE+ (802.3at, ≥30 W) switch or injector.

## Consequences
- Pedals and keyboards connect to the remote device, not to the Pi.
- The Pi gets a fixed address via DHCP reservation; mDNS name as a convenience.
- PoE stability under full load is verified in M4 (`vcgencmd get_throttled`); 27 W USB-C PSU is the fallback.
