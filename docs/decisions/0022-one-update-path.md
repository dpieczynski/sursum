# 0022 — One update path: the server ships inside the system image

Status: Accepted (2026-11-19); built 2026-11-20 (see the amendment), CI image build in the
roadmap section S1. Supersedes the
application-bundle half of ADR 0013 (level 1 and its three amendments on bundles and the trial)
and the sentence "a system update does not change the application" in ADR 0019.

## Context
The device has two complete signed update mechanisms, each with its own file format, upload
endpoint, trial, commit rule, rollback and outcome record:

- application bundles (`.sursum-app`, ADR 0013): `update.rs`, `bundle.rs`, the `bundle guard` in
  `sursum.service`, `releases/` with `current`/`previous`; about 1,100 lines with tests;
- system updates (`.sursum-system`, ADR 0019): `system_update.rs`, three root units, the
  firmware's tryboot; about 1,100 lines with tests.

The remote carries both halves in its update views, the operator guide explains two kinds of
update, a device has two versions (program and system), and the image already contains the
server binary twice: the first release in `/var/lib/sursum/releases/` and the verifier's copy in
`/usr/lib/sursum/sursum-server`. The bundle path came first; the system path was added a day
later because everything outside the binary (kiosk unit, browser, policies, kernel) needed a
reflash. Since then the system path can deliver everything the bundle path can, and the reverse
is not true. An audit on 2026-11-19 ranked this duplication as the largest avoidable part of the
code base.

## Decision
1. **The server is part of the read-only system.** `sursum.service` runs
   `/usr/lib/sursum/sursum-server`. A device has one version: the image build's `git describe`.
2. **`.sursum-system` is the only update file**, uploaded from the remote as today (ADR 0019),
   one entry under "O programie". Removed: `.sursum-app`, `sursum-server bundle
   install|rollback|guard`, `releases/`, `current`/`previous`, the application `trial.json` and
   `update.json`, `POST /api/update`, the program half of the remote's update views,
   `just app-bundle`, `just deploy` and `deploy/pi-manual/` (it installs through a bundle). The
   Ed25519 key handling (`keygen`, signing, `verify_strict`, the compiled-in public key) stays
   and moves next to the system updater.
3. **The database copy and its restore move into the system trial.** Installing takes the
   `before-<version>` copy and records the schema it found; when a trial ends in the old slot,
   the database is put back only if the new system raised the schema, before the old server
   starts (it cannot open a newer schema), and the migrated file is kept in `rolled-back/`.
   These are the bundle guard's rules, and they close the open M7 item "state shared by both
   slots must stay readable by the older slot".
4. **Releases are built by CI**, which becomes possible with the repository public (free arm64
   runners; decided the same day, ADR 0011 amendment): the card image, the `.sursum-system`
   file and the desktop program for Windows.

## Consequences
- An update is about 600 MB and a few minutes (writing the other slot, a restart, the trial)
  instead of 11 MB and a few seconds. Never during Mass, as before. A smaller system (see the
  WPE spike, ADR 0009 amendment of 2026-11-19) would shrink it; that is not measured.
- Every release needs an image build (about 12 minutes locally through QEMU; natively on an
  arm64 runner ⚠ not yet tried).
- Installs that are not our image have no update mechanism. Today that is only the desktop
  program, which never had one (people download a new file).
- Still to decide while building: how a developer tries a server build on a device without a
  full system update (for example a binary under `/var/lib/sursum/dev/` that the unit prefers
  when it exists, reachable only with the administrator's SSH key and named as a development
  build in the remote) ⚠.
- The pilot Pi is the only imaged device: it is reflashed once with a device backup (ADR 0018)
  rather than migrated in place.
- Until this is built, both paths exist and the documents describe them as they are.

## Rejected
- **Keep both and share the plumbing** (manifest and signature checks, the trial record, the
  30 s rule, the upload handling): saves 200 to 300 lines and keeps small fast updates, but also
  keeps two versions, two trials and two entries in the remote.
- **Bundles only**: every kiosk, browser or kernel change is a reflash again, which the
  maintainer found too tiresome even while piloting (ADR 0019).

## Amendment (2026-11-20): built, and the open points decided

- **A developer's build on a device**: `just try <host>` copies the Pi build to `/run` (RAM) and
  points `sursum.service` at it with a runtime drop-in (`/run/systemd/system/…`). Any reboot
  brings back the system's server, so a test build cannot outlive a restart into a service. No
  server code; the remote names it ("Działa wersja testowa …") because the build differs from
  the system's version. The proposed `/var/lib/sursum/dev/` binary was rejected: it survives
  reboots and would need changes in the unit and the remote.
- **The database copy and its restore**: the server copies the database and notes the schema
  when it stages an update (it is the database's user and runs anyway); the root unit carries
  both into the trial. Back in the old slot, `sursum-system-check.service` runs *before*
  `sursum.service`, as the server's user (so every file it writes stays the server's), and puts
  the copy back only if the schema rose. `LastSystemUpdate::RolledBack` says whether it did.
- **The pilot Pi starts clean**: its data predates ADR 0025, which no current build reads, so it
  is reflashed with a new image and set up again (PIN, church, OpenSong import, calibration,
  logo) instead of restoring a device backup.
- **CI builds the image** on a `v*` tag (signed, attached to the release), as an unsigned trial
  in pull requests that change `image/` or `deploy/`, and by hand. The administrator's SSH key
  in released images is the maintainer's public key, the repository variable `SURSUM_SSH_PUBKEY`.
- The update key keeps its names (`SURSUM_BUNDLE_KEY`, `SURSUM_BUNDLE_SIGNING_KEY`): renaming
  them would mean re-entering the secret for no gain. `sursum-server system keygen` replaces
  `bundle keygen`.
