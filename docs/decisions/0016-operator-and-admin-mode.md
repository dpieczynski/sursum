# 0016 — Operator rights by pairing, admin mode by entering the PIN again

Status: Accepted (2026-11-16)

## Context
The plan (§11.1) asks for two roles: operators run the service (navigation, sets, songs) and
admins change the device's setup (import, library merge, calibration, devices). Tablets stay
paired for years, are shared by several people and may be left in the sacristy; an admin role
fixed per device would make a forgotten tablet a permanent admin key.

## Decision
- Pairing with the PIN makes a device an **operator**; nothing else is stored per device.
- **Admin mode** is unlocked on a paired device by entering the same PIN again
  (`POST /api/admin`, the same per-address rate limit as pairing). It lasts 15 minutes, lives
  only in server memory (a restart ends it) and can be ended at once (`DELETE /api/admin`).
- The server enforces rights on every request and every WebSocket command (`Forbidden`), not
  only on connect: `/api/import/opensong`, `/api/library/merge`, `/api/devices*` and the
  `SetScreenProfile`/`Calibrate` commands need admin mode. Exporting the library stays open to
  operators (it is also a backup).
- **Revoking** a device (`POST /api/devices/{id}/revoke`, admin) marks it in the database,
  removes its token from memory and wakes all remote sessions; the revoked one closes.
- Without a PIN (first setup, development) everyone is admin, as before.

## Consequences
One secret (the PIN) to remember; no separate admin password. Whoever knows the PIN can unlock
admin mode on any paired tablet, which matches how parishes share the PIN anyway. A second,
separate admin PIN can be added later as a setting without changing the protocol.
