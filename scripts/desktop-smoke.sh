#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Smoke test of the desktop program (ADR 0020): starts it with an empty data folder and waits
# until the remote in its window has connected to the server, which proves that the window, the
# web view, the embedded remote and the server work together. It reads the program's log, never
# the screen. Needs a display: on a desktop the window shows for a few seconds; in CI run it
# under `dbus-run-session -- xvfb-run -a`.
#
# Usage: scripts/desktop-smoke.sh [path to the sursum binary]
set -euo pipefail
program="${1:-target/release/sursum}"
data="$(mktemp -d)"
trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; rm -rf "$data"' EXIT

XDG_DATA_HOME="$data" RUST_LOG="info,sursum_server::ws=debug" "$program" >"$data/stderr.log" 2>&1 &
pid=$!
log="$data/sursum/sursum.log"
for _ in $(seq 1 60); do
  if grep -q "client connected.*role=Remote" "$log" 2>/dev/null; then
    echo "desktop smoke test passed: the remote connected"
    exit 0
  fi
  if ! kill -0 "$pid" 2>/dev/null; then
    break
  fi
  sleep 1
done
echo "desktop smoke test failed; the program's output:"
cat "$data/stderr.log"
exit 1
