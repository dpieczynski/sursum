# Releasing and updating devices

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
   - builds the Windows pilot binary and attaches both files to a GitHub release with generated
     notes.

The version is the tag without the leading `v`; it becomes the release folder on the device
(`releases/<version>`), so it may contain only letters, digits, `.`, `+` and `-`.

## Updating a device

Never during a service. Each way puts the new release on trial: if it does not run healthily for
30 s within three starts, the device goes back to the previous release by itself (ADR 0013,
second amendment).

- **From the remote** (normal way): Ustawienia → O programie → **Zainstaluj aktualizację**
  (admin mode), choose the `.sursum-app` file. The remote reloads by itself and shows how the
  update ended.
- **Over SSH** (developers): `just deploy <host> <version>` builds, signs (needs
  `SURSUM_BUNDLE_KEY` and `SURSUM_BUNDLE_SIGNING_KEY` locally), copies, installs, restarts and
  waits for the trial to end.
- **USB stick**: not built yet (M4).

## Keys

`sursum-server bundle keygen <file>` makes a key pair: the secret goes to the file (mode 0600),
the public key is printed. The secret lives only in the CI secret and in the maintainer's
`~/.config/sursum/`; it never goes into the repository. Devices trust the key their running
binary was built with, so changing keys needs one release signed with the old key that carries
the new public key.
