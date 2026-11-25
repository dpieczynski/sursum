# Device image (Raspberry Pi 5)

The sursum device is an immutable A/B image built with
[rpi-image-gen](https://github.com/raspberrypi/rpi-image-gen) (ADR 0013), pinned in
`rpi-image-gen.ref`. It is rpi-image-gen's `trixie-minbase-ab` with the `image-rota` layout plus
two layers of ours.

| Path | What |
|---|---|
| `config/sursum.yaml` | the build config: Pi 5, partition sizes, host name `sursum`, admin user `admin` (SSH key only) |
| `layer/sursum-device.yaml` | the server in the read-only system (ADR 0022), its user and unit, `/var/lib/sursum` shared by both slots, Avahi (`sursum.local`), nftables, hardware watchdog, 64M journal, silent black boot, Ethernet only |
| `layer/sursum-kiosk.yaml` | the screen (ADR 0027): WPE WebKit's MiniBrowser on DRM from our rebuilt packages (`just wpe-debs`), the kiosk user and unit, the wait for the server |
| `build.Containerfile` | a Debian trixie build host with the pinned rpi-image-gen, for podman |

Units and the kiosk's wait script are not copied here: the layers install them from
`deploy/systemd/` and `deploy/kiosk/`, so there is one copy of each.

## Building

```sh
just image
```

This builds the server for the Pi with the public update key compiled in (needs the update keys,
see [docs/releasing.md](../docs/releasing.md)), takes the administrator's SSH public key from
`SURSUM_SSH_PUBKEY` (a file, default `~/.ssh/id_ed25519.pub`, the key itself, or `none` for an
image nobody can log in to, as releases are; it is written to `target/image/`, never into the
repository) and runs rpi-image-gen in a privileged podman
container. The version is `git describe` of the checkout. CI builds the same on a tag (roadmap
S1). On an x86_64 machine the
arm64 root filesystem is built through QEMU (`qemu-user-static` with binfmt_misc on the host),
which works but is slow and not officially supported; native arm64 (a Pi 5 or an arm64 CI runner)
is the supported way.

Results land in `target/image/work/`: the image `sursum.img` (and its zstd-compressed copy, SBOM
and manifest in `deploy-*/`) and rpi-image-gen's own A/B bundle `sursum.update.tar.zst`, which
nothing of ours uses (ADR 0019). The system update for devices already running an image is
`target/sursum-system-<system version>.sursum-system`, written and verified by the same command
(see below).

Flash `sursum.img` with Raspberry Pi Imager ("Use custom", and answer "No" to OS customisation)
or `dd`. The image is about 4 GB; on first boot the data partition grows to fill the card, so use a
16 GB card or larger. The image is for the Raspberry Pi 5 only (a Pi 4 blinks seven times: no
kernel for it).

## What the device looks like

- **Partitions**: `bootconfig`, two boot slots, two read-only system slots (erofs, 1500M each),
  and `/persistent` (ext4), which `sursum-grow-persistent` grows to fill the card before its
  first mount. Everything writable is on `/persistent`; `/var` is
  per slot, and `/var/lib/sursum` is shared by both slots, so the database, pictures and
  settings survive a system update. The server is part of each system slot
  (`/usr/lib/sursum/sursum-server`, ADR 0022).
- **First boot**: the empty `/var/lib/sursum` is copied from the image, the server starts, and the
  first remote that opens `http://sursum.local/` sets the PIN and names the church.
- **Screen**: black from power-on (no rainbow, kernel logo, console or cursor) until the page shows
  the parish logo. The display runs at 1920×1080 at 60 Hz: `video=HDMI-A-1:1920x1080@60D` in the
  kernel command line keeps HDMI0 on even without a display, and the browser keeps that mode (it
  draws on DRM directly, with no compositor), also after a hotplug.
- **Network**: Ethernet with DHCP; Wi-Fi and Bluetooth are switched off in `config.txt`. The
  firewall accepts HTTP and SSH from private addresses only, plus mDNS, DHCP and ping. The kiosk
  itself may reach localhost only.
- **Display power and screen guard**: `/etc/sursum/config.toml` (image defaults; the device's own
  `/var/lib/sursum/config.toml` wins) switches the TV on and to standby over HDMI-CEC and lets the
  server restart the kiosk when no screen has been connected for 15 s, through
  `sursum-kiosk-restart.path` (the server itself has no privileges).
- **Updates** (ADR 0019, ADR 0022; the only kind): `just image` also writes
  `target/sursum-system-<version>.sursum-system`. Upload it in the remote (O programie →
  Zainstaluj aktualizację): the server copies the database, the device writes the update to the
  other A/B slot, restarts into it once (tryboot) and keeps it only if the server and the kiosk
  come up healthy; otherwise the next restart brings the old system back by itself, and
  `sursum-system-check` puts the database copy back before the old server starts if the new one
  had raised its schema.
- **Administration**: `ssh admin@sursum.local` with the key given at build time (release images
  have none: add one to `/home/admin/.ssh/authorized_keys` on the `persistent` partition; `/home`
  lives there, so updates keep it, ADR 0026); no password login. A developer runs another server build until the next reboot with `just try <host>`.
- **Logs**: `journalctl` is persistent across reboots (64M).

## Diagnosing a device

- **The screen stays black and nothing answers on the network**: count the green LED's blinks.
  Seven short blinks mean "kernel not found", e.g. this image (Raspberry Pi 5 only) in a Pi 4;
  the [Raspberry Pi documentation](https://www.raspberrypi.com/documentation/computers/configuration.html#led-warning-flash-codes)
  lists the other codes.
- **Read the journal from the card**: the journal is persistent, so it holds every boot that
  reached systemd. Put the card into a Linux laptop, mount the `persistent` partition read-only
  and run `journalctl -D <mount>/log/journal --list-boots`, then `-b -1` and so on.
- **The WPE WebKit packages**: `just image` needs them in `target/wpe-debs`; `just wpe-debs`
  fetches the latest successful build of `.github/workflows/wpewebkit.yml` (an artifact that
  expires after 90 days: run that workflow again before then).
- **A diagnostic image**: `just image y` builds an image that shows the firmware's rainbow
  screen and the kernel's and systemd's messages on the screen (until the kiosk takes it) instead
  of a black boot (layer variable `sursum.diagnostic_boot`). It is otherwise identical; never ship
  it to a church.
