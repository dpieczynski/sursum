# 0024 — One level of rights: a paired remote may do everything

Status: Accepted (2026-11-19). Supersedes the admin mode of ADR 0016 (its Decision points 2 and
3, the second-review note about admin attempts and the admin part of the setup amendment);
pairing, the PIN, the lockouts, revocation and the hardening in ADR 0016 stand.

## Context
ADR 0016 split rights in two: pairing with the PIN made a tablet an operator, and entering the
PIN again unlocked admin mode on it for 15 minutes (imports, merging, calibration, the logo,
device list, updates, restoring a backup, deleting songs). The reason was that a forgotten
tablet should not be a permanent admin key.

In practice the same few people hold the PIN and the tablets, and whoever can pair a tablet can
also unlock admin mode on it. The split cost a second PIN entry for every setup task, a lock on
half of the Ustawienia view, a countdown, a separate per-device lockout, two more endpoints and
a session field, and it showed operators tools they could not use. An audit on 2026-11-19
counted about 300 lines for it; the maintainer decided to remove it and to keep pairing.

## Decision
- A paired remote may do everything the remote offers. There is no admin mode: no second PIN
  entry, no `POST`/`DELETE /api/admin`, no `SessionInfo.admin` or `admin_remaining_s`, no
  `PairOutcome::Unlocked`, no locks in the remote.
- Pairing stays exactly as in ADR 0016: the PIN (6–12 digits, per-address lockout and the global
  budget), the first remote setting the PIN on a new device, `set-pin`, the device list and
  revocation, which closes the revoked remote's sessions at once.
- The server still checks pairing on every request and every WebSocket command; a command from a
  remote that is no longer paired answers `Rejected { code: forbidden }`, and the remote reloads
  into the pairing form.

## Consequences
- A lost or forgotten paired tablet can do everything until it is revoked from another paired
  remote (Ustawienia → Urządzenia). The operator guide says so.
- The device backup still leaves out the PIN and the paired devices (ADR 0018): a backup file
  must not let anyone in.
- A second, separate PIN for setup tasks can come back later only with a new ADR and a real
  need.
