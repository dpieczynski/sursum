# 0016 — Operator rights by pairing, admin mode by entering the PIN again

Status: Accepted (2026-11-16). Admin mode is superseded by ADR 0024 (2026-11-19): a paired
remote may do everything; pairing, the PIN and revocation stand.

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

## Amendment (2026-11-16): hardening after an independent review
- PINs are 6–12 digits; besides the per-address lockout, 30 wrong attempts from all addresses
  within 10 minutes make every PIN entry wait (a guest can change addresses at will).
- The server answers only to IP addresses, `localhost`, `*.local` and `allowed_hosts` from
  `config.toml` (DNS rebinding); file uploads need the `X-Sursum-Upload` header, which cross-site
  pages cannot send without a CORS preflight — this matters while no PIN is set.
- Uploaded archives are read through hard size caps (64 MB per entry, 256 MB per upload,
  50 000 entries); hashes in `.sursum` files are recomputed, never trusted; pictures are capped at
  2 GB in total and written atomically even when the same picture is uploaded twice at once.
- If the state actor ever panics, the process exits so the service manager restarts it (the
  screen keeps its slide and is restored within the same boot).
- Known and accepted for now: a hand-edited `.sursum` history can fast-forward over a local edit (the old version stays in
  `song_history`); a large merge runs on the state actor and briefly delays navigation, so merge
  outside services.
- After a second review: unlocking admin mode on a paired tablet is rate-limited per device and
  is not part of the global pairing budget, so guests guessing at the pairing form cannot lock
  the parish out of admin mode.

## Amendment (2026-11-16): no open device without a PIN
- A device without a PIN no longer lets everyone in as admin: nobody may control the screen, and
  the remote shows a "set the PIN" form instead (PIN entered twice). `POST /api/setup` accepts it
  only while no PIN is set — the first claim wins atomically, later ones get `already_set` — and
  pairs that browser in admin mode so the installer can go on (import, calibration, logo).
  `sursum-server set-pin` still works for installers.
- The old open behaviour remains only behind the explicit `--open-without-pin` flag (CLI only,
  never from `config.toml`), for `just dev` and the tests.
- Remaining risk: whoever reaches a freshly installed device first can set its PIN; the installer
  sets it right after installation (or before, with `set-pin`).
