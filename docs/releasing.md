# Releasing and updating devices

A device has one version: the image build's `git describe`. The server is part of the read-only
system (`/usr/lib/sursum/sursum-server`), and a signed `.sursum-system` file is the only way to
update a device (ADR 0019, ADR 0022).

## Cutting a release

1. Make sure `main` is green in CI and the roadmap is up to date.
2. Tag the commit on `main` and push the tag:
   `git tag -s v0.2.0 -m "sursum 0.2.0" && git push origin v0.2.0`.
3. The `Release` workflow (`.github/workflows/release.yml`) runs in the `release` environment
   (only `main` and `v*` tags may use it). It:
   - builds the device's card image and its signed system update natively on an arm64 runner
     (`just image`, below), with the public key from the repository variable `SURSUM_BUNDLE_KEY`
     compiled in and the secret `SURSUM_BUNDLE_SIGNING_KEY` (written to a file readable only by
     the runner user, deleted at the end of the step); without an SSH key (ADR 0026);
   - checks the update against the compiled-in key, so a secret that does not match the variable
     fails the release instead of producing updates every device refuses;
   - builds the desktop program for Windows (`sursum-<version>-windows-x86_64.exe`, ADR 0020) on
     a Windows runner with MSVC (one file, the WebView2 loader linked in; unsigned, see the
     operator guide on SmartScreen);
   - attaches `sursum-<version>.img.zst` (flash it), `sursum-system-<version>.sursum-system`
     (upload it in the remote), `sursum-<version>.packages.txt` and
     `sursum-<version>.spdx.json.zst` (what is in the image, for the sources offer in
     [sources.md](sources.md)) and the Windows program to a GitHub release with generated notes.

Pull requests that change `image/`, `deploy/`, the justfile or the system updater also build the
image (`.github/workflows/image.yml`), signed with a throwaway key made for that run, so a broken
layer shows up before a release.

## Building the image and the update

```sh
set -a; . ~/.config/sursum/bundle-key.pub; set +a
export SURSUM_BUNDLE_SIGNING_KEY=~/.config/sursum/bundle-signing.key
just image
```

This builds the server for the Pi with the public key compiled in, the card image
`target/image/work/image-sursum/sursum.img` and the update
`target/sursum-system-<version>.sursum-system`, and checks the update against the key, so a
secret that does not match the public key fails the build instead of producing an update every
device refuses. Details of the image: [image/README.md](../image/README.md).

## Updating a device

Never during a service. In the remote: Ustawienia → O programie → **Zainstaluj aktualizację**,
choose the `.sursum-system` file. Then:

1. The server checks the signature, copies the database (`backups/*-before-<version>.db`) and
   notes its schema, and asks the root unit to install.
2. The root unit writes the update to the other A/B slot with the running system's server and
   restarts into it once (tryboot). The remote shows the steps and reconnects by itself.
3. The new system is on trial: if the server and the kiosk are healthy for 30 s within three
   minutes, it becomes the default. Otherwise the device restarts and the firmware starts the old
   slot again.
4. Back in the old slot, before the old server starts, `sursum-system-check` puts the database
   copy back if the new server had already raised the schema (the old one could not open it);
   the migrated database is kept in `rolled-back/`, and the remote says that changes made during
   the trial are gone.

There is no USB stick path (ADR 0013, amendment of 2026-11-17) and no smaller "application only"
update any more (ADR 0022): an update is about 600 MB and takes a few minutes.

## Trying a server build on a device (developers)

```sh
just try admin@sursum.local
```

builds the server for the Pi, copies it to `/run` on the device (RAM) and points `sursum.service`
at it with a runtime drop-in. The remote shows "Działa wersja testowa …". Any reboot brings back
the system's own server. A build that raises the database schema leaves a database the system's
server refuses after that reboot: put back a copy from before the try (`backups/`) or install a
system update with the new server. Never leave a test build running before a service.

## Keys

`sursum-server system keygen <file>` makes a key pair: the secret goes to the file (mode 0600),
the public key is printed. The secret lives only in the `release` environment's CI secret
`SURSUM_BUNDLE_SIGNING_KEY` and in the maintainer's `~/.config/sursum/`; it never goes into the
repository. The public key is the repository variable `SURSUM_BUNDLE_KEY`. (The names come from
the application bundles this key first signed, ADR 0013; the key stayed when they were removed.)
Devices trust the key their running server was built with, so changing keys needs one update
signed with the old key that carries a server with the new public key.

Locally, `~/.config/sursum/bundle-key.pub` holds the line `SURSUM_BUNDLE_KEY=<hex>`: load it
with `set -a; . ~/.config/sursum/bundle-key.pub; set +a` rather than exporting the file's
contents.

A locally built image carries the administrator's SSH public key (login `admin`, key only):
`SURSUM_SSH_PUBKEY` (default `~/.ssh/id_ed25519.pub`; `none` for no key). Release images carry
none (ADR 0026). The key lands in `/home` on the persistent partition, so system updates keep it;
on a device flashed from a release image, add it to `/home/admin/.ssh/authorized_keys` on the
card's `persistent` partition (owner `admin`, mode 0600) from a Linux computer.
