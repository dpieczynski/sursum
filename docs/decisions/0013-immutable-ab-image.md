# 0013 — Immutable A/B device image built with rpi-image-gen

Status: Accepted as the preliminary direction (2026-11-16); revisit only with a new ADR.
The application bundles (update level 1 and the amendments about bundles and their trial) are
superseded by ADR 0022 (2026-11-19, not built yet); the image itself stands.
Release images carry no administrator SSH key (ADR 0026, 2026-11-22).

## Context
The device must survive sudden power loss and accept updates, at least of our own software.
"Raspberry Pi OS plus `apt install` plus an overlay root" is hand-assembled, not reproducible,
and gives no safe update or rollback path. Alternatives considered: containers (podman) on the
device, Yocto, Buildroot, Ubuntu Core, balenaOS.

## Decision
Build our own **immutable device image** with
[rpi-image-gen](https://github.com/raspberrypi/rpi-image-gen) (official Raspberry Pi tool,
BSD-3-Clause), based on `trixie-minbase-ab` with the **`image-rota`** layout:
- read-only A/B system slots (erofs) and A/B boot partitions; no package installs on the device;
- all writable state on the `/persistent` ext4 partition; `/var/lib/sursum` is declared
  slot-shared (`/etc/rpi-image-gen/slot-shared.d/sursum.conf`), the journal is persistent;
- our layers live in `image/` (`sursum-kiosk`: cage + chromium + seatd + kiosk unit, starting
  from the official `webkiosk` example; `sursum-device`: users, nftables, Avahi, udev for
  `/dev/cec0`, watchdog, cmdline/config, splash screen).

Two update levels:
1. **Application** (frequent, from M4): signed bundle `.sursum-app` (tar.zst: binary with
   embedded frontend, manifest, Ed25519 signature; public key compiled into the binary) installed
   into `/var/lib/sursum/releases/<ver>/` with an atomic `current` switch. Channels: browser upload
   in the admin panel, USB stick (confirmed with the PIN), `just deploy` over SSH for developers.
   Database snapshot before migrations; automatic rollback (binary + matching DB snapshot) if
   `/healthz` fails within 30 s. Independent of the OS slot (binary built for glibc 2.36).
2. **System** (rare, M7): the rpi-image-gen `.update.tar.zst` written to the inactive slot, trial
   boot via Raspberry Pi `tryboot`, commit after a healthy `/healthz`, otherwise automatic
   rollback. The built-in OTA agent currently depends on Raspberry Pi Connect (cloud); if it
   cannot apply bundles locally offline, use **RAUC** (1.13 in Trixie) on the same A/B layout.

Images are built on native Debian arm64 (supported host): a GitHub arm64 runner with a privileged
Debian Trixie container, or a spare Pi 5. QEMU builds on x86 laptops are a convenience only.

## Rejected / deferred
- **Pi OS + apt + overlayfs**: not reproducible, no safe update/rollback.
- **Podman on the device**: the server is a single dependency-free binary, so a container
  isolates nothing; it adds an image store (SD writes) and makes the kiosk harder (DRM, GPU, seat,
  input passthrough). rpi-image-gen's `oci_create` example keeps an OCI export possible later.
- **Yocto**: full control and native read-only rootfs, but Chromium (meta-browser) builds for
  hours, we would lose Raspberry Pi's Chromium acceleration patches, and maintaining layers is a
  large ongoing cost for one developer. Reconsider if we leave Raspberry Pi hardware or
  rpi-image-gen is abandoned.
- **Buildroot**: no Chromium (WPE only), conflicts with ADR 0009.
- **Ubuntu Core / balenaOS**: updates tied to an external store or cloud; we work offline.

## Consequences
- M4 delivers the image (no `setup-pi.sh` on a stock OS); M1 spikes may still use stock Pi OS Lite.
- x86_64 mini-PC image later with `mkosi` + `systemd-sysupdate`, same principles and same
  application bundle format.
- The rpi-image-gen version is pinned (submodule or commit in CI).
- Distributing images requires providing sources (or a written offer) for GPL packages
  (ADR 0010).

## Amendment (2026-11-16): application bundles as built
- Container: **ZIP** instead of tar.zst — the server already uses `zip` (deflate) for library files
  and OpenSong imports, so no tar and zstd crates are added, and single entries can be read with
  hard size limits. Entries: `manifest.json` (format, version, build id, SHA-256 of the binary),
  `manifest.sig` (hex Ed25519 signature of the manifest bytes), `sursum-server`.
- Signing with `ed25519-dalek` 3.0 (BSD-3-Clause, `verify_strict`). The public key is compiled in
  from `SURSUM_BUNDLE_KEY` at build time; builds without it refuse every bundle. The secret key is
  created by `sursum-server bundle keygen` (file mode 0600) and stays outside the repository (CI
  secret).
- `sursum-server bundle install <file>` verifies, backs up the database (`before-<version>`),
  writes `releases/<version>/sursum-server` atomically and switches the relative symlinks
  `current`/`previous` by rename; `bundle rollback` swaps them back. Still to do on the device (M4):
  systemd units running `current/sursum-server`, the restart, the `/healthz` check with automatic
  rollback, and the browser/USB upload paths.
- After a second review: each release records the database schema it found
  (`releases/<v>/installed.json`); `bundle rollback` refuses once the newer release has migrated
  the database (the older program could not read it) and points to the `before-<version>`
  backup instead. The database is quarantined and restored from a backup only on real damage,
  never because its schema is newer. Old releases are pruned (only `current` and `previous` stay),
  half-written files are removed on failure, and `bundle install` backs up through a plain
  connection, so it never migrates or "repairs" the database under a running server.

## Amendment (2026-11-17): the trial of a new release
The "automatic rollback if `/healthz` fails within 30 s" is built as a **trial** that does not
depend on the new binary being able to judge itself:
- Installing (CLI `bundle install` or `POST /api/update` from the remote in admin mode) writes
  `trial.json` next to `current`: the release on trial, the release to go back to, the
  `before-<version>` database copy and the schema before the update.
- The unit runs `ExecStartPre=-/var/lib/sursum/previous/sursum-server … bundle guard` before every
  start: the **previous** release, known good, counts the starts. After the third start without a
  commit it switches `current` back, restores the database copy only if the new release had
  already raised the schema (the migrated file is kept in `rolled-back/`), removes the failed
  release so it never runs again, and records the outcome in `update.json`. Crashes, start
  timeouts (`TimeoutStartSec=30`), watchdog kills and power cuts all count as starts;
  `StartLimitIntervalSec=0` keeps systemd from giving up before the guard does.
- The new server commits itself (removes `trial.json`, writes `update.json`) once it has run for
  30 s with its state actor answering — and only if the running executable is the release on
  trial, so the old server cannot vouch for its successor before the restart.
- An upload from the remote is accepted only by builds with a bundle key that run under a
  service manager (`NOTIFY_SOCKET`, i.e. our `Type=notify` unit); the server answers, then exits
  and systemd starts the new release. The remote reloads by itself (new build id) and shows the
  outcome under "O programie".
- Data written during a failed trial is lost only when the schema changed (the database goes back
  to the pre-update copy); otherwise the database is kept as it is.
- A first install has nothing to go back to; the image ships a working release.
- `trial.json` is read by the previous release, so its fields are only ever added with defaults.
Verified on a desktop with a systemd user unit: a release that exits at once and one that never
reports ready both went back by themselves after three starts (about 5 s and 3 × the start
timeout), and a good release committed after 30 s.

## Amendment (2026-11-17): no USB stick path
The maintainer decided that application updates from a USB stick are not needed: the remote
uploads the same signed `.sursum-app` file from any tablet or PC, with the same trial and
rollback. The image therefore gets no automount or udev rule for updates; the two paths are the
remote and `just deploy` over SSH.

## Amendment (2026-11-17): the first image, as built
`image/` holds the build (see `image/README.md`), checked against rpi-image-gen v2.9.0 (pinned in
`image/rpi-image-gen.ref`):
- `trixie-minbase-ab` + `image-rota` as decided; system slots 1500M (cage, Wayland and Chromium do
  not fit the base 512M), data partition 512M in the image. rpi-image-gen does not grow it, so
  `sursum-grow-persistent` does, at every boot before `/persistent` is checked and mounted
  (`sfdisk`, `partx`, `e2fsck`, `resize2fs`; a no-op once it fills the card). Growing it while
  mounted was tried first and made systemd stop everything bound to the re-probed disk.
- Two layers: `sursum-device` (server user and unit, first release, slot-shared `/var/lib/sursum`,
  Avahi, nftables, watchdog, 64M journal, silent boot, Ethernet only) and `sursum-kiosk`. Units,
  Chromium policies and the cursor generator are installed from `deploy/` (one copy, shared with
  the hand-installed pilot); the kiosk's default browser command lives in its unit, because files
  seeded into slot-shared `/var/lib/sursum` are never overwritten by a later image.
- The first release is installed at build time by the bundle's own binary (through QEMU on x86),
  exactly like a later install, and goes on trial on first boot.
- Maintainer decisions (2026-11-17): administration by **SSH key only** (user `admin`, key passed
  at build time, never in the repository; no password); **Ethernet only** (Wi-Fi and Bluetooth off
  in `config.txt`; the base's systemd-networkd does DHCP); **black** from power-on to the page (no splash image).
- Resolution (decided later the same day): HDMI0 is **forced to 1920×1080 at 60 Hz**
  (`video=HDMI-A-1:1920x1080@60D`). The pilot on a 4K TV came up at the TV's preferred
  3840×2160 at 30 Hz, which halves the frames of a fade and quadruples the pixels every full-screen
  layer blends. A forced mode also brings the screen up when the projector is switched on after
  the Pi or sits behind an extender with a broken EDID. The screen draws a fixed 1920×1080 stage
  anyway, a 4K display upscales 1080p exactly 2×, and lyrics are read from metres away, so native
  resolution would add little. Native or another forced mode (e.g. a 1920×1200 projector) only if
  an installation needs it.
  On the Pi the kernel parameter alone was not enough: it sets the console's mode and keeps the
  connector on without a display, but cage always takes the display's preferred mode (the LG TV:
  3840×2160 at 30 Hz again). The kiosk therefore runs inside cage through `kiosk-session`, which
  sets `SURSUM_SCREEN_MODE` (default `1920x1080@60`, `native` to leave it) with `wlr-randr`
  before Chromium starts and checks it every 5 s, so a TV switched on later or a replugged cable
  gets the mode back. The variable can be overridden per device in `/var/lib/sursum/kiosk.env`.
- Networking stays with the base's systemd-networkd: rpi-image-gen allows only one
  `network-activator`, and NetworkManager is not needed for Ethernet.
- Builds run in a privileged Debian trixie podman container (`image/build.Containerfile`,
  `just image <version>`); on x86_64 through QEMU, which rpi-image-gen calls unsupported but
  working. The repository is private, so GitHub's free arm64 runners are not available; CI builds
  wait.
