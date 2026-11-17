#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Writes the kiosk's transparent cursor themes into <root>/usr/share/sursum/cursors.
#
#   sh blank-cursors.sh /          (on a running system: deploy/pi-manual/install.sh)
#   sh blank-cursors.sh "$rootfs"  (into an image being built: image/layer/sursum-kiosk.yaml)
#
# cage has no option to hide its cursor and draws it in the middle of the screen even without a
# mouse; the page hides it only once a pointer moves over it, which never happens without one.
# After a page reload Chromium sets its own cursor too (e.g. "progress"), from whatever theme it
# picks. The kiosk unit therefore gets a private cursor path (XCURSOR_PATH) in which the themes
# sursum-blank, default and Adwaita all hold only a transparent cursor under every known name.
# The rest of the system keeps its cursors.
#
# The cursor is an Xcursor file, little endian: header (magic, header size 16, version 1.0, one
# table entry), table entry (image, nominal size 24, at byte 28), image chunk (header size 36,
# image, size 24, version 1, 1 x 1 pixels, hotspot 0,0, no delay) and one transparent pixel.
set -eu

root=${1:?usage: blank-cursors.sh <root directory>}
themes=$root/usr/share/sursum/cursors
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
if [ -d "$root/usr/share/icons/Adwaita/cursors" ]; then
    names="$names $(ls "$root/usr/share/icons/Adwaita/cursors")"
fi
for name in $names; do
    [ "$name" = default ] || ln -sf default "$cursors/$name"
done
printf '[Icon Theme]\nName=sursum-blank\n' > "$themes/sursum-blank/index.theme"
ln -sfn sursum-blank "$themes/default"
ln -sfn sursum-blank "$themes/Adwaita"
