# Releasing and updating devices

> ADR 0022 (2026-11-19) decides that the server will ship inside the system image and that
> `.sursum-system` will be the only update file. It is not built yet (roadmap S1); this page
> describes releases as they work today and will be rewritten with that change.

## Cutting a release

1. Make sure `main` is green in CI and the roadmap is up to date.
2. Tag the commit on `main` and push the tag:
   `git tag -s v0.2.0 -m "sursum 0.2.0" && git push origin v0.2.0`.
3. The `Release` workflow (`.github/workflows/release.yml`) runs in the `release` environment
   (only `main` and `v*` tags may use it). It:
   - builds the server for the Pi (`aarch64`, glibc 2.36) with both web apps embedded and the
     public key from the repository variable `SURSUM_BUNDLE_KEY` compiled in;
   - signs `sursum-<version>.sursum-app` with the `SURSUM_BUNDLE_SIGNING_KEY` secret (written to a
     temporary file readable only by the runner user, deleted at the end of the step);
   - checks the bundle against the compiled-in key (`sursum-server bundle verify`), so a secret
     that does not match the variable fails the release instead of producing bundles every device
     refuses;
   - takes the desktop program for Windows (`sursum-<version>-windows-x86_64.exe`, ADR 0020),
     which a separate job builds on a Windows runner with MSVC (one file, the WebView2 loader
     linked in; unsigned, see the operator guide on SmartScreen);
   - attaches the two files to a GitHub release with generated notes.

The version is the tag without the leading `v`; it becomes the release folder on the device
(`releases/<version>`), so it may contain only letters, digits, `.`, `+` and `-`.

## Updating a device

Never during a service. Each way puts the new release on trial: if it does not run healthily for
30 s within three starts, the device goes back to the previous release by itself (ADR 0013,
second amendment).

- **From the remote** (normal way): Ustawienia → O programie → **Zainstaluj aktualizację**
  choose the `.sursum-app` file. The remote reloads by itself and shows how the
  update ended.
- **Over SSH** (developers): `just deploy <host> <version>` builds, signs (needs
  `SURSUM_BUNDLE_KEY` and `SURSUM_BUNDLE_SIGNING_KEY` locally), copies, installs, restarts and
  waits for the trial to end.
- There is no USB stick path (ADR 0013, amendment of 2026-11-17).

An application bundle replaces only the server binary with its embedded pages. Everything else
on an imaged device (the kiosk unit, Chromium and its policies, cage, the kernel) arrives as a
**system update** (ADR 0019): `just image <version>` writes
`target/sursum-system-<system version>.sursum-system` next to the card image, signed with the
same key; upload it in the remote under Ustawienia → O programie → **Zainstaluj aktualizację
systemu**. The device writes the other A/B slot, restarts into it once and keeps it
only if the server and the kiosk come up healthy. The release workflow does not build images
yet (roadmap M4, M7), so system updates are built locally.

## Keys

`sursum-server bundle keygen <file>` makes a key pair: the secret goes to the file (mode 0600),
the public key is printed. The secret lives only in the CI secret and in the maintainer's
`~/.config/sursum/`; it never goes into the repository. Devices trust the key their running
binary was built with, so changing keys needs one release signed with the old key that carries
the new public key.

Locally, `~/.config/sursum/bundle-key.pub` holds the line `SURSUM_BUNDLE_KEY=<hex>`: load it
with `set -a; . ~/.config/sursum/bundle-key.pub; set +a` rather than exporting the file's
contents, and point `SURSUM_BUNDLE_SIGNING_KEY` at `~/.config/sursum/bundle-signing.key`.
`just app-bundle` verifies the result, so a malformed key fails the build instead of producing a
release that would accept no later update.
