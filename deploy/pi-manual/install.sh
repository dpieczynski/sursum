#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Hand installation of sursum on a Raspberry Pi 5 with Raspberry Pi OS Lite (64-bit), for piloting
# until the device image exists (ADR 0013, M4). See README.md next to this script.
#
#   sudo sh deploy/pi-manual/install.sh sursum-0.1.0.sursum-app
#
# Safe to run again: users, folders and units are only created when missing, and an already
# installed release is left alone (later updates go through the remote).
set -eu

bundle=${1:?usage: sudo sh install.sh sursum-<version>.sursum-app}
here=$(cd "$(dirname "$0")" && pwd)
data=/var/lib/sursum

[ "$(id -u)" = 0 ] || { echo "run with sudo" >&2; exit 1; }
[ "$(uname -m)" = aarch64 ] || { echo "needs a 64-bit OS on the Pi (aarch64)" >&2; exit 1; }
[ -f "$bundle" ] || { echo "no such file: $bundle" >&2; exit 1; }

echo "== packages"
apt-get update
apt-get install -y --no-install-recommends cage chromium seatd unzip curl

echo "== users and folders"
id sursum >/dev/null 2>&1 || useradd --system --home-dir "$data" --shell /usr/sbin/nologin sursum
id sursum-kiosk >/dev/null 2>&1 ||
    useradd --system --create-home --home-dir /var/lib/sursum-kiosk --shell /usr/sbin/nologin sursum-kiosk
# seatd hands the display to the video group; input and render for the GPU and devices.
usermod -a -G video,render,input sursum-kiosk
install -d -o sursum -g sursum -m 0750 "$data"

echo "== application"
if [ -e "$data/current" ]; then
    echo "a release is installed already ($(readlink "$data/current")); update it from the remote"
else
    # The first install runs the bundle's own binary, which checks the bundle against the key it
    # was built with. Trust comes from downloading the bundle from the project's GitHub release.
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    cp "$bundle" "$work/bundle.sursum-app"
    unzip -q "$work/bundle.sursum-app" sursum-server -d "$work/bin"
    chmod 0755 "$work/bin/sursum-server"
    chown -R sursum:sursum "$work"
    runuser -u sursum -- "$work/bin/sursum-server" --data-dir "$data" bundle install "$work/bundle.sursum-app"
fi

if [ ! -e "$data/kiosk.env" ]; then
    cat > "$data/kiosk.env" <<'EOF'
# The browser cage starts (ADR 0009). Restart with: sudo systemctl restart sursum-kiosk
KIOSK_APP=/usr/bin/chromium --ozone-platform=wayland --kiosk --noerrdialogs --disable-session-crashed-bubble --hide-crash-restore-bubble --incognito --no-first-run --disable-features=Translate --disable-pinch --overscroll-history-navigation=0 --check-for-update-interval=31536000 --password-store=basic --user-data-dir=/run/sursum-kiosk/chromium http://localhost/screen/
EOF
    chown sursum:sursum "$data/kiosk.env"
fi

echo "== invisible cursor"
# cage has no option to hide its cursor and draws it in the middle of the screen even without a
# mouse; the page hides it only once a pointer enters. A cursor theme whose only image is one
# transparent pixel hides it everywhere. Xcursor file, little endian: header (magic, header size
# 16, version 1.0, one table entry), table entry (image, nominal size 24, at byte 28), image
# chunk (header size 36, image, size 24, version 1, 1 x 1 pixels, hotspot 0,0, no delay) and one
# fully transparent ARGB pixel.
cursors=/usr/share/icons/sursum-blank/cursors
install -d -m 0755 "$cursors"
{
    printf 'Xcur\020\000\000\000\000\000\001\000\001\000\000\000'
    printf '\002\000\375\377\030\000\000\000\034\000\000\000'
    printf '\044\000\000\000\002\000\375\377\030\000\000\000\001\000\000\000'
    printf '\001\000\000\000\001\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000'
    printf '\000\000\000\000'
} > "$cursors/default"
chmod 0644 "$cursors/default"
for name in left_ptr text pointer hand2 xterm; do
    ln -sf default "$cursors/$name"
done
printf '[Icon Theme]\nName=sursum-blank\n' > /usr/share/icons/sursum-blank/index.theme

echo "== services"
install -m 0644 "$here/../systemd/sursum.service" /etc/systemd/system/sursum.service
install -m 0644 "$here/sursum-kiosk.service" /etc/systemd/system/sursum-kiosk.service
systemctl daemon-reload
systemctl disable --now getty@tty1.service 2>/dev/null || true
systemctl enable --now seatd.service sursum.service
systemctl enable sursum-kiosk.service
# Restart so a changed unit or cursor theme takes effect when the script is run again.
systemctl restart sursum-kiosk.service

echo "== waiting for the server"
for _ in $(seq 30); do
    if curl -fsS http://localhost/healthz >/dev/null 2>&1; then
        echo "ready: open http://$(hostname).local/ on the tablet; the first remote sets the PIN"
        exit 0
    fi
    sleep 1
done
echo "the server did not answer; see: journalctl -u sursum -b" >&2
exit 1
