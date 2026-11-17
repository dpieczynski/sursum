# deploy and image — local rules

Target: Raspberry Pi 5 next to the projector, Ethernet (optionally PoE), running **our own
immutable A/B image** built with rpi-image-gen from `image/` (ADR 0013). Also a Windows pilot
setup (`deploy/windows/`).

- **Never assume a stock OS with `apt install`.** Packages and system configuration belong in the
  image layers under `image/layer/`; the device's root filesystem is read-only (A/B slots). The one
  exception is `deploy/pi-manual/`, an interim hand install on Raspberry Pi OS Lite for piloting
  until the image exists; keep it small and never let the image depend on it.
- Everything that must persist lives in **`/var/lib/sursum`**, which is slot-shared on the
  `/persistent` partition: database, backups, assets, `config.toml`, `kiosk.env`, and the app in
  `releases/<version>/` with a `current` symlink. Anything else written at runtime is lost on a
  slot change, by design.
- **Application updates** are signed `.sursum-app` bundles (Ed25519): upload from the remote or
  `just deploy` for developers (no USB stick: ADR 0013 amendment of 2026-11-17). Always: DB
  snapshot before migration, trial with automatic rollback. Never deploy during a service.
- **System updates** (M7) go through the A/B slots with a trial boot and automatic rollback.
- **Silent boot**: from power-on the HDMI output may show only black, logo or content. Any change
  to `cmdline.txt`, `config.txt`, the splash screen or the kiosk unit must be verified on a real
  Pi (ideally with an HDMI capture) before merging.
- HDMI mode is forced (`video=HDMI-A-1:1920x1080@60D`); do not rely on hotplug or EDID.
- The display is connected directly to the Pi's **HDMI0** port (`/dev/cec0` for HDMI-CEC power
  control, ADR 0012). The `sursum` user gets access to `/dev/cec0` via a udev rule in the image;
  PJLink passwords live only in `/var/lib/sursum/config.toml`.
- After an OS reboot the screen starts with the logo and the set position restored; after a
  process restart the screen is fully restored (ADR 0003).
- The kiosk command line is read from `/var/lib/sursum/kiosk.env`, so the display engine can be
  changed without rebuilding (ADR 0009). The image layer supports `kiosk.engine = chromium | cog`
  (ADR 0015). For cog always pass `--bg-color=black` (default is white) and
  `--webprocess-failure=exit` (default shows an error page); prefer `--platform=drm` (no cage).
- Pin the rpi-image-gen version. Never commit secrets (PINs, signing keys, certificates); the
  bundle signing key lives outside the repo (CI secret).
