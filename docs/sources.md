# Sources of the software in the device image

The device image (`sursum-<version>.img.zst`) and the system update
(`sursum-system-<version>.sursum-system`) contain, besides sursum itself, free software from
Debian 13 (trixie) and the Raspberry Pi OS archive: the Linux kernel, the boot firmware, systemd,
glibc, WPE WebKit and others. Many of them are licensed under the GNU GPL or similar licences,
which require that their source code is available to everyone who receives the binaries.

## Which packages

Every release lists them:

- `sursum-<version>.packages.txt`: each package and its exact version, one per line;
- `sursum-<version>.spdx.json.zst`: the full SPDX bill of materials of the image's file system.

## Where the sources are

- **sursum** itself: this repository, at the release's tag (GPL-3.0-or-later).
- **Debian packages** (versions without `+rpt`, `+rpi` or `~deb13sursum`):
  `apt-get source <package>=<version>` on Debian 13, or from
  [snapshot.debian.org](https://snapshot.debian.org), which keeps every version Debian has
  published.
- **WPE WebKit** (versions with `~deb13sursum`): Debian's own `wpewebkit` source package from sid,
  rebuilt unchanged for trixie by `.github/workflows/wpewebkit.yml` (its `VERSION` names the
  Debian version); the source is that Debian version, from snapshot.debian.org.
- **Raspberry Pi packages** (versions with `+rpt` or `+rpi`, the kernel and the firmware): the
  source packages in [archive.raspberrypi.com](https://archive.raspberrypi.com/debian/), and the
  kernel and firmware repositories under [github.com/raspberrypi](https://github.com/raspberrypi).

## Written offer

For at least three years after a release of sursum, the maintainer will give anyone who asks
the complete corresponding source code of every package in that release's image and system
update, for no more than the cost of providing it (normally a download link, free). To ask, open
an issue at [github.com/dpieczynski/sursum/issues](https://github.com/dpieczynski/sursum/issues)
naming the release and the packages.
