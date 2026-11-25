# deploy and image — local rules

Target: Raspberry Pi 5 next to the projector, Ethernet (optionally PoE), running **our own
immutable A/B image** built with rpi-image-gen from `image/` (ADR 0013). The image is the only
way to run the screen (ADR 0023).

- **Never assume a stock OS with `apt install`.** Packages and system configuration belong in the
  image layers under `image/layer/` (they install units and kiosk files from `deploy/`, so there is
  one copy of each); the device's root filesystem is read-only (A/B slots).
- Everything that must persist lives in **`/var/lib/sursum`**, which is slot-shared on the
  `/persistent` partition: database, backups, assets, `config.toml`, `kiosk.env`, update state
  (`system-update/`). Anything else written at runtime is lost on a slot change, by design.
- **One update path** (ADR 0022): the server is part of the read-only system
  (`/usr/lib/sursum/sursum-server`), so a device has one version. Updates are signed
  (Ed25519) `.sursum-system` files (`just image` writes one; ADR 0019), uploaded from the remote,
  written to the other A/B slot and started once through tryboot, with automatic rollback; the
  database copy taken before the update goes back if a failed system had raised its schema
  (`sursum-system-check.service`, before the server). No USB stick (ADR 0013 amendment of
  2026-11-17). Never update during a service.
- **Developers** try a server build with `just try <host>`: the binary in `/run` and a runtime
  drop-in, gone at the next reboot. Never leave one running before a service.
- **Silent boot**: from power-on the HDMI output may show only black, logo or content (black
  until the page draws; there is no splash image). Any change to `cmdline.txt`, `config.txt` or
  the kiosk unit must be verified on a real Pi (ideally with an HDMI capture) before merging.
- HDMI mode is forced (`video=HDMI-A-1:1920x1080@60D`, decided 2026-11-17, ADR 0013 amendment)
  and the browser keeps the kernel's mode (WPE's DRM platform has no mode option). Do not rely on
  hotplug or EDID. Another mode for an installation means another kernel command line.
- The display is connected directly to the Pi's **HDMI0** port (`/dev/cec0` for HDMI-CEC power
  control, ADR 0012). The server unit reaches `/dev/cec0` (`root:video`) through
  `SupplementaryGroups=video`; the image turns CEC on in `/etc/sursum/config.toml`;
  PJLink passwords live only in `/var/lib/sursum/config.toml`.
- After an OS reboot the screen starts with the logo and the set position restored; after a
  process restart the screen is fully restored (ADR 0003).
- The kiosk's default command line lives in its unit (`deploy/kiosk/sursum-kiosk.service`);
  `KIOSK_APP=` in `/var/lib/sursum/kiosk.env` overrides it per device without a new image. The
  engine is WPE WebKit's MiniBrowser on the DRM platform, no compositor (ADR 0027), from our trixie
  rebuild of Debian's 2.54 source package (`.github/workflows/wpewebkit.yml`; `just wpe-debs`
  fetches it for `just image`). Debian's package does not declare everything it needs at run time,
  so the layer names `libwpebackend-fdo-1.0-1`, `bubblewrap`, `xdg-dbus-proxy` and `libgles2` and
  checks them at build time. The kiosk unit may reach localhost only (`IPAddressAllow=localhost`,
  `IPAddressDeny=any`) and starts the browser only once `/healthz` answers
  (`ExecStartPre=/usr/lib/sursum/wait-for-server`), then opens `/screen/` directly: a browser sent
  to a server that is down shows its error page, and WebKit does not let a local page ask the
  server first. A `KIOSK_APP` override keeps that wait (it is in the unit, not the command).
- Pin the rpi-image-gen version. Never commit secrets (PINs, signing keys, certificates); the
  update signing key lives outside the repo (CI secret `SURSUM_BUNDLE_SIGNING_KEY`).
