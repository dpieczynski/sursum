# SPDX-License-Identifier: GPL-3.0-or-later
# WPE WebKit 2.54 and its WebDriver for the screen's end-to-end tests (ADR 0015, ADR 0027): WebKit's
# own MiniBrowser, the browser the device runs, here headless. Debian forky's amd64 build of the
# same `wpewebkit` source package that `.github/workflows/wpewebkit.yml` rebuilds for the device
# (trixie itself has only 2.48, and forky's packages need forky's libc, so this is a separate image
# from sursum-browsers). Forky follows sid: when sid moves past the device's version, so do these
# tests, until the device is rebuilt too.
# Run with host networking so the browser can reach a sursum server on 127.0.0.1:
#   podman run --rm --network host localhost/sursum-wpe WPEWebDriver --port=9516
FROM docker.io/library/debian:forky-slim

# MiniBrowser needs libWPEBackend-fdo, which the package does not declare (the device's kiosk layer
# names it too); Mesa provides software GL for the renderer.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      wpewebkit-webdriver libwpewebkit-2.0-1 libwpebackend-fdo-1.0-1 \
      libgles2 libegl1 libgl1-mesa-dri \
      fonts-dejavu-core ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && test -x /usr/lib/x86_64-linux-gnu/wpe-webkit-2.0/MiniBrowser \
 && dpkg-query -W libwpewebkit-2.0-1

# The headless platform needs a runtime directory.
ENV XDG_RUNTIME_DIR=/tmp
