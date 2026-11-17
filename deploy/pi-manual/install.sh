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

echo "== screen browser"
# What Chromium must not do on a screen (translate, sign in, phone home, update components, ...)
# is set by managed policies, Chromium's supported mechanism; the command line keeps only what
# has no policy. --disable-background-networking stops the background requests no policy covers
# (e.g. push messaging registration).
install -d -m 0755 /etc/chromium/policies/managed
install -m 0644 "$here/chromium-policies.json" /etc/chromium/policies/managed/sursum.json
# kiosk.env is ours to update while it carries the marker line; delete that line to keep a
# command of your own (ADR 0009: the engine can be changed without rebuilding).
marker="# Managed by deploy/pi-manual/install.sh; delete this line to keep your own command."
# The first version of this script wrote this command without the marker; it is ours too.
first="KIOSK_APP=/usr/bin/chromium --ozone-platform=wayland --kiosk --noerrdialogs --disable-session-crashed-bubble --hide-crash-restore-bubble --incognito --no-first-run --disable-features=Translate --disable-pinch --overscroll-history-navigation=0 --check-for-update-interval=31536000 --password-store=basic --user-data-dir=/run/sursum-kiosk/chromium http://localhost/screen/"
if [ ! -e "$data/kiosk.env" ] || grep -qxF -e "$marker" -e "$first" "$data/kiosk.env"; then
    cat > "$data/kiosk.env" <<EOF
$marker
# The browser cage starts (ADR 0009). Restart with: sudo systemctl restart sursum-kiosk
KIOSK_APP=/usr/bin/chromium --ozone-platform=wayland --kiosk --noerrdialogs --disable-session-crashed-bubble --hide-crash-restore-bubble --incognito --no-first-run --disable-background-networking --disable-pinch --overscroll-history-navigation=0 --password-store=basic --user-data-dir=/run/sursum-kiosk/chromium http://localhost/screen/
EOF
    chown sursum:sursum "$data/kiosk.env"
else
    echo "keeping your own $data/kiosk.env"
fi

echo "== invisible cursor"
# cage has no option to hide its cursor and draws it in the middle of the screen even without a
# mouse; the page hides it only once a pointer moves over it, which never happens without one.
# After a page reload Chromium sets its own cursor too (e.g. "progress"), from whatever theme it
# picks. The kiosk therefore gets a private cursor path (XCURSOR_PATH, see sursum-kiosk.service)
# in which the themes sursum-blank, default and Adwaita all hold only a transparent cursor under
# every known name. The rest of the system keeps its cursors.
#
# The cursor is an Xcursor file, little endian: header (magic, header size 16, version 1.0, one
# table entry), table entry (image, nominal size 24, at byte 28), image chunk (header size 36,
# image, size 24, version 1, 1 x 1 pixels, hotspot 0,0, no delay) and one transparent pixel.
rm -rf /usr/share/icons/sursum-blank # left by an earlier version of this script
themes=/usr/share/sursum/cursors
cursors=$themes/sursum-blank/cursors
install -d -m 0755 "$cursors"
{
    printf 'Xcur\020\000\000\000\000\000\001\000\001\000\000\000'
    printf '\002\000\375\377\030\000\000\000\034\000\000\000'
    printf '\044\000\000\000\002\000\375\377\030\000\000\000\001\000\000\000'
    printf '\001\000\000\000\001\000\000\000\000\000\000\000\000\000\000\000\000\000\000\000'
    printf '\000\000\000\000'
} > "$cursors/default"
chmod 0644 "$cursors/default"
# CSS and legacy X11 names, plus every name the installed Adwaita theme has.
names="alias all-resize all-scroll arrow bd_double_arrow bottom_left_corner bottom_right_corner
bottom_side cell center_ptr circle closedhand col-resize color-picker context-menu copy cross
crosshair cross_reverse diamond_cross dnd-ask dnd-copy dnd-link dnd-move dnd-no-drop dnd-none
dotbox e-resize ew-resize fd_double_arrow fleur grab grabbing half-busy hand1 hand2 help ibeam
left_ptr left_ptr_watch left_side link move n-resize ne-resize nesw-resize no-drop not-allowed
ns-resize nw-resize nwse-resize openhand pencil pirate plus pointer progress question_arrow
right_ptr right_side row-resize s-resize sb_h_double_arrow sb_v_double_arrow se-resize
size_all size_bdiag size_fdiag size_hor size_ver split_h split_v sw-resize target tcross text
top_left_arrow top_left_corner top_right_corner top_side vertical-text w-resize wait watch
whats_this X_cursor xterm zoom-in zoom-out"
if [ -d /usr/share/icons/Adwaita/cursors ]; then
    names="$names $(ls /usr/share/icons/Adwaita/cursors)"
fi
for name in $names; do
    [ "$name" = default ] || ln -sf default "$cursors/$name"
done
printf '[Icon Theme]\nName=sursum-blank\n' > "$themes/sursum-blank/index.theme"
ln -sfn sursum-blank "$themes/default"
ln -sfn sursum-blank "$themes/Adwaita"

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
