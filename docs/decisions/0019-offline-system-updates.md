# 0019 — Offline system updates through the firmware's tryboot

Status: Accepted (2026-11-17)

## Context
The device image (ADR 0013) has a read-only A/B system. Application bundles (`.sursum-app`) only
replace the server binary with its embedded pages; the kiosk unit, Chromium's policies, the
cursor theme, the server unit, Chromium, cage and the kernel belong to the system. Without
system updates, every such change meant reflashing the card, which the maintainer found too
tiresome even while piloting. rpi-image-gen builds an A/B update bundle (`<name>.update.tar.zst`
with sparse `boot` and `system` images) but the only agent that applies one is Raspberry Pi
Connect's remote update, which needs the cloud; churches may have no internet. ADR 0013 named
RAUC as the fallback.

## Decision
Our own small updater on the Raspberry Pi firmware's **tryboot** (`autoboot.txt` on the
`bootconfig` partition: `boot_partition` is started normally, `[tryboot]`'s partition once after
`reboot "0 tryboot"`), and on rpi-image-gen's `/dev/disk/by-slot/{active,other}` links.

- **File**: `.sursum-system`, a ZIP with `manifest.json` (format, version = the image build's
  `git describe`, size and SHA-256 of each image), `manifest.sig` (Ed25519, the application
  bundles' key and code) and the raw `boot.img` (vfat) and `system.img` (erofs) of an image
  build. Raw images need no sparse-image, tar or zstd code on the device. `just image` writes it
  next to the card image and verifies it.
- **Upload**: from the remote, "O programie" → "Zainstaluj aktualizację systemu" (admin), only on
  devices running the image (`/etc/sursum/system-release`) under systemd with a bundle key. The
  server streams the file into `system-update/` on the data partition, verifies it and creates
  `/run/sursum/system-update`. The server stays unprivileged.
- **Install** (root, `sursum-system-update.path` → `.service`): runs the read-only system's own
  copy of the server (`/usr/lib/sursum/sursum-server`, so a compromised server, which can replace
  its own releases, cannot plant a system). It verifies again, writes both images to the
  **other** slot (checking every byte against the manifest), records `trial.json`, writes
  `autoboot.txt` with the running slot as default and the other as `[tryboot]`, and reboots with
  `0 tryboot`. A failure before the reboot is recorded and the device keeps running.
- **Trial** (root, `sursum-system-finish.service`, every boot): in the new slot it waits up to
  three minutes for the server's `/healthz` and the kiosk, requires 30 s of both, then makes the
  slot the default (`commit`). If they don't come up, it reboots, which starts the old slot. A
  hang is caught by the hardware watchdog and a kernel panic by `panic=10`. Either way the
  firmware starts the default (old) slot, since tryboot lasts one boot. Back in the old slot, the
  trial is recorded as rolled back. The remote shows the system version and how the last update
  ended.
- `/var/lib/sursum` (data and application releases) is shared by both slots and untouched; a
  system update does not change the application, and an application update does not need one.

## Rejected
- **Raspberry Pi Connect's agent**: needs the cloud.
- **RAUC**: a second update system with its own bundle format, keys and bootloader integration
  (it would need a custom backend for tryboot anyway). Our updater is a few hundred lines on the
  same signing code and is tested end to end with fake slots.
- **Writing rpi-image-gen's `.update.tar.zst`** on the device: sparse images and tar/zstd for no
  gain over raw images in a ZIP (the erofs image is compressed already).

## Consequences
- A system update is about 600 MB and takes a few minutes, including writing the other slot on
  the SD card. Never during Mass; the remote warns.
- The first image with this updater must still be flashed once; every later system arrives over
  the network.
- ⚠ Not yet run on a Pi: writing a real slot, `systemctl reboot "0 tryboot"`, the commit after a
  healthy trial, and the rollback after a deliberately broken system.
