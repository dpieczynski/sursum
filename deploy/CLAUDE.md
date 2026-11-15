# deploy — local rules

Target: Raspberry Pi 5, Raspberry Pi OS Lite 64-bit (Debian Trixie), next to the projector,
Ethernet (optionally PoE). Also a Windows pilot setup (`deploy/windows/`).

- **Root filesystem is read-only** (overlay). Everything that must persist lives on the `/data`
  partition: database, backups, assets, config, and the app itself in
  `/data/sursum/releases/<version>/` with a `current` symlink.
- **Silent boot**: from power-on the HDMI output may show only black, logo or content. Any change
  to `cmdline.txt`, `config.txt` or the kiosk unit must be verified on a real Pi (ideally with an
  HDMI capture) before merging.
- HDMI mode is forced (`video=HDMI-A-1:1920x1080@60D`); do not rely on hotplug or EDID.
- After an OS reboot the screen starts with the logo and the set position restored; after a
  process restart the screen is fully restored (ADR 0003).
- The kiosk command line is read from `/data/sursum/kiosk.env`, so the display engine can be
  changed without rebuilding (ADR 0009).
- Deploy = copy binary to a new release dir, switch the symlink, restart, check `/healthz`, roll
  back on failure. Never deploy during a service.
- Never commit secrets (PINs, keys, certificates).
