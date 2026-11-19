# 0022 — One update path: the server ships inside the system image

Status: Accepted (2026-11-19); not built yet, see the roadmap section S1. Supersedes the
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
