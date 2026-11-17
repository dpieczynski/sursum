# Device image (Raspberry Pi 5)

The sursum device is an immutable A/B image built with
[rpi-image-gen](https://github.com/raspberrypi/rpi-image-gen) (ADR 0013), pinned in
`rpi-image-gen.ref`. It is rpi-image-gen's `trixie-minbase-ab` with the `image-rota` layout plus
two layers of ours.

| Path | What |
|---|---|
| `config/sursum.yaml` | the build config: Pi 5, partition sizes, host name `sursum`, admin user `admin` (SSH key only) |
| `layer/sursum-device.yaml` | server user and unit, the first application release, `/var/lib/sursum` shared by both slots, Avahi (`sursum.local`), nftables, hardware watchdog, 64M journal, silent black boot, Ethernet only |
| `layer/sursum-kiosk.yaml` | cage + Chromium + seatd, the kiosk user and unit, Chromium managed policies, transparent cursors |
| `build.Containerfile` | a Debian trixie build host with the pinned rpi-image-gen, for podman |

Units, policies and the cursor generator are not copied here: the layers install them from
`deploy/systemd/` and `deploy/kiosk/`, which the hand-installed pilot (`deploy/pi-manual/`) uses
too.

## Building

```sh
just image 0.1.3
```

This builds and signs the application bundle `target/sursum-0.1.3.sursum-app` (needs the bundle
keys, see [docs/releasing.md](../docs/releasing.md)), takes the administrator's SSH public key from
`SURSUM_SSH_PUBKEY` (default `~/.ssh/id_ed25519.pub`; it is copied to `target/image/`, never into
the repository) and runs rpi-image-gen in a privileged podman container. On an x86_64 machine the
arm64 root filesystem is built through QEMU (`qemu-user-static` with binfmt_misc on the host),
which works but is slow and not officially supported; native arm64 (a Pi 5 or an arm64 CI runner)
is the supported way.

Results land in `target/image/work/`: the image `sursum.img` (and its zstd-compressed copy, SBOM
and manifest in `deploy-*/`) and the A/B update bundle `sursum.update.tar.zst` for later system
updates (M7).

Flash `sursum.img` with Raspberry Pi Imager ("Use custom", and answer "No" to OS customisation)
or `dd`. The image is about 4 GB; on first boot the data partition grows to fill the card, so use a
16 GB card or larger. The image is for the Raspberry Pi 5 only (a Pi 4 blinks seven times: no
kernel for it).

## What the device looks like

- **Partitions**: `bootconfig`, two boot slots, two read-only system slots (erofs, 1500M each),
  and `/persistent` (ext4), which `sursum-grow-persistent` grows to fill the card before its
  first mount. Everything writable is on `/persistent`; `/var` is
  per slot, and `/var/lib/sursum` is shared by both slots, so the database, pictures and
  application releases survive a system update.
- **First boot**: `/var/lib/sursum` is copied from the image (release on trial, committed after
  30 s of healthy running), the server starts, and the first remote that opens
  `http://sursum.local/` sets the PIN.
- **Screen**: black from power-on (no rainbow, kernel logo, console or cursor) until the page shows
  the parish logo. The resolution is the screen's preferred mode (open question in the roadmap).
- **Network**: Ethernet with DHCP; Wi-Fi and Bluetooth are switched off in `config.txt`. The
  firewall accepts HTTP and SSH from private addresses only, plus mDNS, DHCP and ping. The kiosk
  itself may reach localhost only.
- **Administration**: `ssh admin@sursum.local` with the key given at build time; no password
  login. Application updates come from the remote (on trial, ADR 0013) or `just deploy`.
- **Logs**: `journalctl` is persistent across reboots (64M).
