# SPDX-License-Identifier: GPL-3.0-or-later
# Browsers and WebDriver servers for the end-to-end tests: Chromium (ADR 0014) and WebKitGTK for the
# remote on WebKit (ADR 0021). The screen's engine, WPE WebKit, is in wpe.Containerfile.
# Run with host networking so the browsers can reach a sursum server on 127.0.0.1:
#   podman run --rm --network host localhost/sursum-browsers chromedriver --port=9515
FROM docker.io/library/debian:trixie-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      chromium chromium-driver \
      webkitgtk-webdriver libwebkitgtk-6.0-4 xvfb dbus-x11 \
      libgles2 libegl1 libgl1-mesa-dri \
      fonts-dejavu-core ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# WebKitGTK needs a runtime directory; Mesa (above) provides software GL for its renderer.
ENV XDG_RUNTIME_DIR=/tmp
COPY webkitgtk-webdriver.sh /usr/local/bin/webkitgtk-webdriver
