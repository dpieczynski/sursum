# SPDX-License-Identifier: GPL-3.0-or-later
# A Debian trixie build host for rpi-image-gen (ADR 0013), so the image can be built on any Linux
# with podman. Native arm64 hosts are rpi-image-gen's supported setup; on x86_64 it builds through
# QEMU (binfmt_misc with the F flag registered on the host, e.g. Fedora's qemu-user-static), which
# rpi-image-gen calls "not officially supported" but working. Run it privileged and as root (then
# rpi-image-gen does its chroot work directly instead of through podman). `just image` uses it.
FROM docker.io/library/debian:trixie

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates git sudo binfmt-support \
      qemu-user-static debian-archive-keyring \
 && rm -rf /var/lib/apt/lists/*

# The pinned rpi-image-gen (image/rpi-image-gen.ref: tag and commit).
ARG RIG_COMMIT
RUN git clone https://github.com/raspberrypi/rpi-image-gen.git /opt/rpi-image-gen \
 && git -C /opt/rpi-image-gen checkout --detach "$RIG_COMMIT" \
 && apt-get update && cd /opt/rpi-image-gen && ./install_deps.sh \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src
