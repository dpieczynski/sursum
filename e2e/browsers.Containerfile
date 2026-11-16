# SPDX-License-Identifier: GPL-3.0-or-later
# Browsers and WebDriver servers for the end-to-end tests (ADR 0014, ADR 0015).
# Run with host networking so the browsers can reach a sursum server on 127.0.0.1:
#   podman run --rm --network host localhost/sursum-browsers chromedriver --port=9515
FROM docker.io/library/debian:trixie-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      chromium chromium-driver \
      cog wpewebkit-webdriver \
      libgles2 libegl1 libgl1-mesa-dri \
      fonts-dejavu-core ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Headless WPE needs a runtime directory; Mesa (above) provides software GL for its renderer.
ENV XDG_RUNTIME_DIR=/tmp
