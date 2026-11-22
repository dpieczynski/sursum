# 0026 — Public release images: no administrator key, a package list and a sources offer

Status: Accepted (2026-11-22). Changes ADR 0013's "SSH key passed at build time" for the images
published on GitHub; carries out ADR 0010's and ADR 0013's requirement to provide the sources of
the GPL software in the image.

## Context
The release workflow put the maintainer's SSH public key into every published card image, for
the user `admin`, who may use `sudo` without a password. Anyone who flashed a public image gave
that key root access from their local network, without being told. The firewall limits SSH to
private addresses, but the device is someone else's then.

The image and the `.sursum-system` update also distribute Debian and Raspberry Pi packages under
the GPL and other copyleft licences (the Linux kernel, glibc, systemd, Chromium and others), and
no release came with their sources or an offer for them.

On the device, `/home` is the persistent partition's `/persistent/home`. It is filled from the
image when the card is flashed and never touched by a system update, which carries only the boot
and system partitions.

## Decision
- **Public images carry no SSH key.** The release workflow builds with `SURSUM_SSH_PUBKEY=none`,
  which leaves `authorized_keys` empty; password login stays off. An owner who wants SSH puts a
  key into `/home/admin/.ssh/authorized_keys` on the card's `persistent` partition (ext4) from a
  Linux computer, or builds the image locally with `just image`, which still takes
  `SURSUM_SSH_PUBKEY` (default `~/.ssh/id_ed25519.pub`). Because `/home` is on the persistent
  partition, the key survives every later update, public ones too: the parish devices keep the
  maintainer's access.
- **Every release lists its packages and offers their sources.** The release attaches
  `sursum-<version>.packages.txt` (rpi-image-gen's manifest: every Debian and Raspberry Pi
  package with its exact version) and `sursum-<version>.spdx.json.zst` (rpi-image-gen's SPDX
  bill of materials). [docs/sources.md](../sources.md) says where the sources of those versions
  are published and includes a written offer, valid for three years after a release, to provide
  them. The release notes point to it.
- The release workflow pins every action to a commit and grants `contents: write` only to the
  job that publishes, since that job holds the key every device trusts.

## Consequences
- A device flashed from a public image cannot be administered over SSH until its owner adds a key.
  This is intentional; the remote does everything a parish needs.
- The maintainer answers source requests, by pointing to the archives or, if a version is no
  longer there, by sending the sources. Debian keeps every published version on
  snapshot.debian.org; the Raspberry Pi archive keeps fewer, so the release that ships a version
  is where the offer matters most.
- Images built by pull requests (`image.yml`) are artifacts for the maintainer's own testing, not
  releases; they keep the key from the repository variable.
