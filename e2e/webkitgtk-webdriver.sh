#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# WebKitGTK's WebDriver for the remote on WebKit (roadmap P2, ADR 0021: the desktop program's
# Linux window is WebKitGTK). Its MiniBrowser is a real GTK window whose size WebDriver can set; it
# needs a display (Xvfb) and a session bus. Usage: webkitgtk-webdriver.sh [port] (default 9517).
set -eu
port="${1:-9517}"
Xvfb :99 -screen 0 2560x1600x24 -nolisten tcp >/tmp/xvfb.log 2>&1 &
export DISPLAY=:99
eval "$(dbus-launch --sh-syntax)"
export GTK_A11Y=none
exec WebKitWebDriver --port="$port"
