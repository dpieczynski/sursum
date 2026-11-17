# 0018 — Whole-device backup and restore from the remote

Status: Accepted (2026-11-17)

## Context
The `.sursum` library file carries only songs, and it is *merged*: it can never bring a
device back to an earlier state. Sets, the logo, pictures, calibration and settings lived only
on the SD card (plus the automatic database copies in `backups/` on the same card). A broken
card, or a new device for the same church, meant setting everything up again. The maintainer
asked for a full backup (2026-11-17).

## Decision
- A **`.sursum-backup`** file is a ZIP with `manifest.json` (format, creation time, build id),
  `sursum.db` (a `VACUUM INTO` copy) and `assets/<id>` for every stored picture. It carries the
  songs with their history, sets, settings, calibration, the logo and the program position.
- **No credentials travel**: the export removes the PIN hash and the paired devices from the
  copy (then `VACUUM`s it, so no deleted pages linger). On restore the device **keeps its own**
  PIN and paired remotes, so the remote that restores stays paired. A new device is first
  paired with a new PIN, then restored.
- `GET /api/backup` (any paired remote, like the library export) streams the backup from a
  temporary file in the data directory, removed once sent. `POST /api/backup/restore` (admin
  mode, `X-Sursum-Upload`) streams the upload to a temporary file (cap: picture quota + 1 GiB),
  checks it (manifest format, `application_id`, schema not newer than this program,
  `integrity_check`), adds the pictures to the content-addressed store at once (that changes
  nothing that is shown), stages the database in `restore/`, answers a `BackupSummary`
  (date, songs, sets, pictures) and restarts like an update.
- The **next start** moves the staged database into place before opening it: a copy of the
  replaced database goes to `backups/` (`…-before-restore.db`), the staged one is migrated if
  it is older, given this device's credentials, checkpointed and renamed over `sursum.db`. A
  staged backup that cannot be applied is discarded and the server starts on its current data.
  Replacing everything under the running state actor would need every part of it to reload;
  a restart already exists for updates and is simpler to reason about.
- Restoring needs a service manager (as updates do); elsewhere it is refused (`not_supervised`).
- The remote offers both under Ustawienia → **Kopia zapasowa urządzenia**, with a warning and a
  confirmation before restoring, and reloads after it.

## Consequences
- One file brings a device back after a broken card or a mistake; the parish can keep it on the
  office computer. It contains the library, so it is shared only like the library file.
- A backup from the other church can be restored too; it replaces everything (the `.sursum`
  merge stays the way to share songs).
- Pictures stay in the store even when the restored database no longer uses them (as after
  any deletion today); a picture clean-up is a separate task.
- A restore during an update's trial is not guarded against: a rollback of that update would
  bring back the pre-update database. Both are admin actions done outside Mass.
