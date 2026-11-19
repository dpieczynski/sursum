# deploy and image — local rules

Target: Raspberry Pi 5 next to the projector, Ethernet (optionally PoE), running **our own
immutable A/B image** built with rpi-image-gen from `image/` (ADR 0013). The image is the only
way to run the screen (ADR 0023).

- **Never assume a stock OS with `apt install`.** Packages and system configuration belong in the
  image layers under `image/layer/` (they install units and kiosk files from `deploy/`, so there is
  one copy of each); the device's root filesystem is read-only (A/B slots). The one
  exception is `deploy/pi-manual/`, the interim hand install on Raspberry Pi OS Lite from before
  the image existed (the image has run on the pilot Pi 5 since 2026-11-17); do not extend it and
  never let the image depend on it.
- Everything that must persist lives in **`/var/lib/sursum`**, which is slot-shared on the
  `/persistent` partition: database, backups, assets, `config.toml`, `kiosk.env`, and the app in
  `releases/<version>/` with a `current` symlink. Anything else written at runtime is lost on a
  slot change, by design.
- **One update path is decided but not built** (ADR 0022): the server moves into the read-only
  system and `.sursum-system` becomes the only update file. Do not extend the bundle path below.
- **Application updates** are signed `.sursum-app` bundles (Ed25519): upload from the remote or
  `just deploy` for developers (no USB stick: ADR 0013 amendment of 2026-11-17). Always: DB
  snapshot before migration, trial with automatic rollback. Never deploy during a service.
- **System updates** (ADR 0019) are signed `.sursum-system` files (`just image` writes one),
  uploaded from the remote, written to the other A/B slot and started once through tryboot, with
  automatic rollback. Everything outside the server binary (kiosk unit, browser, policies,
  kernel) changes only this way or by reflashing.
- **Silent boot**: from power-on the HDMI output may show only black, logo or content (black
  until the page draws; there is no splash image). Any change to `cmdline.txt`, `config.txt` or
  the kiosk unit must be verified on a real Pi (ideally with an HDMI capture) before merging.
- HDMI mode is forced (`video=HDMI-A-1:1920x1080@60D`, decided 2026-11-17, ADR 0013 amendment);
  cage ignores the kernel parameter for its own mode, so `deploy/kiosk/kiosk-session` sets and
  keeps `SURSUM_SCREEN_MODE` with `wlr-randr`. Do not rely on hotplug or EDID. Native or another forced mode only if an installation needs it.
- The display is connected directly to the Pi's **HDMI0** port (`/dev/cec0` for HDMI-CEC power
  control, ADR 0012). The server unit reaches `/dev/cec0` (`root:video`) through
  `SupplementaryGroups=video`; the image turns CEC on in `/etc/sursum/config.toml`;
  PJLink passwords live only in `/var/lib/sursum/config.toml`.
- After an OS reboot the screen starts with the logo and the set position restored; after a
  process restart the screen is fully restored (ADR 0003).
- The kiosk's default command line lives in its unit (`deploy/kiosk/sursum-kiosk.service`);
  `KIOSK_APP=` or `SURSUM_SCREEN_MODE=` in `/var/lib/sursum/kiosk.env` override it per device
  without a new image (ADR 0009). The engine is Chromium under cage; what Chromium must not
  do goes into **managed policies** (`/etc/chromium/policies/managed/`), the command line keeps
  only what no policy covers, cage gets a transparent cursor theme, and the kiosk unit may reach
  localhost only (`IPAddressAllow=localhost`, `IPAddressDeny=any`; all in `deploy/kiosk/`).
  cog is legacy and not used on devices. A spike decides whether WPE WebKit 2.54 on DRM (our
  rebuild of Debian's source package, WebKit's own MiniBrowser, no cage) replaces all of this
  (ADR 0009 amendment of 2026-11-19).
- Pin the rpi-image-gen version. Never commit secrets (PINs, signing keys, certificates); the
  bundle signing key lives outside the repo (CI secret).
