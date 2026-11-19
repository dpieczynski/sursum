# Hand-installed pilot on a Raspberry Pi 5

An interim way to run sursum on a Pi 5 from before the device image existed (ADR 0013, M4). The
image (`image/`, `just image <version>`) has run on the pilot Pi 5 since 2026-11-17 and is the
way to set up a device now. It uses a
stock Raspberry Pi OS, so it has none of the image's guarantees: the root filesystem is writable,
boot is not silent (the rainbow and console are visible), the HDMI mode is not forced, there is no
firewall or kiosk watchdog. Use it to try the release, the remote and the screen, not for Mass.

## You need

- Raspberry Pi 5, SD card (16 GB or more), HDMI cable to the TV or projector (port **HDMI0**,
  next to the USB-C power), Ethernet or Wi-Fi.
- On your computer: Raspberry Pi Imager, the `.sursum-app` file of a release
  (`gh release download v0.1.0 -p '*.sursum-app'` or the release page on GitHub) and this
  repository's `deploy/` folder.

## Steps

1. **Flash** *Raspberry Pi OS Lite (64-bit)* with Raspberry Pi Imager. In its settings set a host
   name (e.g. `sursum`), a user with a password, Wi-Fi if needed, and enable SSH.
2. **Boot** the Pi with the screen connected and wait until it is on the network
   (`ping sursum.local`).
3. **Copy** the bundle and the deploy folder:
   `scp -r deploy sursum-0.1.0.sursum-app <user>@sursum.local:`
4. **Install**: `ssh <user>@sursum.local`, then
   `sudo sh deploy/pi-manual/install.sh sursum-0.1.0.sursum-app`.
   The script installs `cage`, `chromium` and `seatd`, creates the users `sursum` (server) and
   `sursum-kiosk` (screen), installs the release into `/var/lib/sursum`, sets Chromium's managed
   policies (`chromium-policies.json` → `/etc/chromium/policies/managed/sursum.json`: no
   translation, sign-in, sync, metrics, component updates and the like), writes
   `/var/lib/sursum/kiosk.env` (the short command line; kept if you removed its first line),
   installs a transparent cursor theme for the kiosk, installs `sursum.service` (from
   `deploy/systemd/`) and `sursum-kiosk.service`, and starts both. It ends with the address for
   the remote.
5. **Pair**: open `http://sursum.local/` on the tablet. The first remote sets the PIN.

The script is safe to run again. Once a release is installed it leaves it alone: later versions go
through the remote (Ustawienia → O programie → Zainstaluj aktualizację).

## What to check

1. The screen shows black, then the logo; the remote connects; slides fade smoothly; fonts fit.
2. Updates: install a newer release from the remote. It should restart, and "O programie" should
   say it works after about 30 s. A release that fails to start must go back by itself after three
   starts (`journalctl -u sursum -b` shows the guard).
3. Backup: Ustawienia → Dane urządzenia → Pobierz kopię, then Przywróć z kopii. The tablet stays
   paired; note how long the restart takes.
4. Power cuts: pull the plug during a slide. After boot the screen shows the logo and the set
   position is kept.

## Useful commands

| What | Command |
|---|---|
| Server log | `journalctl -u sursum -b` |
| Screen log | `journalctl -u sursum-kiosk -b` |
| Restart the screen | `sudo systemctl restart sursum-kiosk` |
| Installed releases | `ls -l /var/lib/sursum/` (`current`, `previous`, `releases/`) |
| How the last update ended | `cat /var/lib/sursum/update.json` |
| Go back to the previous release | `sudo -u sursum /var/lib/sursum/current/sursum-server --data-dir /var/lib/sursum bundle rollback && sudo systemctl restart sursum` |

Run on the pilot Pi 5 on 2026-11-17 (roadmap M4): the screen and the remote worked with these
flags under cage, with no cursor, no browser UI and no background requests after the network
block. If the screen stays black, start with `journalctl -u sursum-kiosk -b` and `journalctl -u seatd -b`.
