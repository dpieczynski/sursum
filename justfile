# SPDX-License-Identifier: GPL-3.0-or-later
# Developer commands. Run `just` to list them.

export SURSUM_BUILD_ID := `git describe --always --dirty 2>/dev/null || echo dev`

# List recipes.
default:
    @just --list

# Build both web apps (WebAssembly) with Trunk.
build-web:
    cd apps/screen && trunk build --release
    cd apps/remote && trunk build --release

# Build the release server with both apps embedded.
build: build-web
    cargo build --release -p sursum-server

# Run the server and both apps with auto-reload (server :8080, remote :8081, screen :8082).
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT
    cargo run -p sursum-server -- --data-dir sursum-data --listen 127.0.0.1:8080 --open-without-pin \
        --allow-origin http://127.0.0.1:8081 --allow-origin http://localhost:8081 \
        --allow-origin http://127.0.0.1:8082 --allow-origin http://localhost:8082 &
    (cd apps/remote && trunk serve) &
    (cd apps/screen && trunk serve) &
    wait

# Unit and integration tests (native).
test:
    cargo test --workspace
    cargo test -p sursum-slide -p sursum-client -p sursum-remote

# Build the image with browsers and WebDriver servers for the end-to-end tests.
browsers-image:
    podman build -t localhost/sursum-browsers -f e2e/browsers.Containerfile e2e

# Start chromedriver (:9515) and WPEWebDriver (:9516) in the background, on localhost only.
# `--init` reaps finished browser processes; without it they pile up as zombies until new
# sessions fail ("Timed out receiving message from renderer").
browsers:
    podman run -d --rm --init --name sursum-chromedriver --network host localhost/sursum-browsers chromedriver --port=9515
    podman run -d --rm --init --name sursum-wpedriver --network host localhost/sursum-browsers WPEWebDriver --port=9516

# Stop the WebDriver containers.
browsers-stop:
    -podman stop sursum-chromedriver sursum-wpedriver

# Browser tests of the font fitting in the shared drawing crate (needs `just browsers`).
test-wasm:
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner CHROMEDRIVER_REMOTE=http://127.0.0.1:9515 \
        cargo test -p sursum-slide --target wasm32-unknown-unknown --test dom

# End-to-end tests: everything on Chromium, then the screen tests on WPE WebKit (ADR 0015).
e2e: build-web
    cargo build -p sursum-server --features e2e-hooks
    SURSUM_E2E_BROWSER=chromium cargo test -p sursum-e2e -- --ignored --test-threads=1
    SURSUM_E2E_BROWSER=wpe cargo test -p sursum-e2e -- --ignored --test-threads=1

# Screenshots of the screen and the remote, for a quick visual check.
screenshots dir="screenshots": build-web
    cargo build -p sursum-server
    cargo run -p sursum-e2e --example screenshots -- {{dir}}

# Screenshots of the remote on a phone, an upright and a landscape tablet (operator and admin).
remote-sizes dir="remote-sizes": build-web
    cargo build -p sursum-server
    cargo run -p sursum-e2e --example remote_sizes -- {{dir}}

# Regenerate the third-party license notices served at /licenses (commit the result).
licenses:
    cargo about generate --fail -m crates/sursum-desktop/Cargo.toml -o crates/sursum-server/assets/licenses-server.html about.hbs
    cargo about generate --fail -m apps/remote/Cargo.toml -o crates/sursum-server/assets/licenses-web.html about.hbs
    sed -i 's/\r$//' crates/sursum-server/assets/licenses-*.html

# Fail if the committed license notices are out of date.
licenses-check: licenses
    git diff --exit-code -- crates/sursum-server/assets

# Formatting, lints (native and wasm32), SPDX headers and dependency policy.
lint:
    cargo fmt --all --check
    leptosfmt --check crates/sursum-slide apps
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p sursum-slide -p sursum-client -p sursum-screen -p sursum-remote --target wasm32-unknown-unknown -- -D warnings
    scripts/check-spdx.sh
    cargo deny check

# Cross-compile the server for the Raspberry Pi (needs zig and cargo-zigbuild).
cross-pi: build-web
    cargo zigbuild --release -p sursum-server --target aarch64-unknown-linux-gnu.2.36

# ZIP of the public-domain hymn library (library/public-domain) for Ustawienia → Import z OpenSong.
library-zip:
    rm -f target/biblioteka-domena-publiczna.zip
    mkdir -p target
    cd library/public-domain && zip -qr -X ../../target/biblioteka-domena-publiczna.zip Songs
    @echo "target/biblioteka-domena-publiczna.zip"

# Signed application bundle for the Pi (ADR 0013): needs SURSUM_BUNDLE_KEY (public, compiled in)
# and SURSUM_BUNDLE_SIGNING_KEY (path of the secret key file, never in the repository).
app-bundle version: cross-pi
    test -n "$SURSUM_BUNDLE_KEY" && test -n "$SURSUM_BUNDLE_SIGNING_KEY"
    cargo run -q -p sursum-server -- bundle create --key "$SURSUM_BUNDLE_SIGNING_KEY" --version {{version}} \
        target/aarch64-unknown-linux-gnu/release/sursum-server target/sursum-{{version}}.sursum-app
    # A malformed SURSUM_BUNDLE_KEY compiles in no key: the device would install the bundle but
    # accept no update after it. Checking with the same key catches that and a mismatched secret.
    cargo run -q -p sursum-server -- bundle verify target/sursum-{{version}}.sursum-app

# Device image for a Raspberry Pi 5 (ADR 0013, image/) with release <version> as its first
# application; `just image <version> y` makes a diagnostic image (boot messages instead of black).
# Also writes the matching system update target/sursum-system-<system version>.sursum-system
# (ADR 0019), for devices already running an image; the system version is `git describe`. Needs the bundle keys (see app-bundle) and the administrator's SSH public key
# (SURSUM_SSH_PUBKEY, default ~/.ssh/id_ed25519.pub; never kept in the repository). Builds with
# the pinned rpi-image-gen in a privileged podman container; on x86_64 through QEMU (slow).
image version diagnostic="n": (app-bundle version)
    #!/usr/bin/env bash
    set -euo pipefail
    key="${SURSUM_SSH_PUBKEY:-$HOME/.ssh/id_ed25519.pub}"
    test -f "$key" || { echo "no SSH public key at $key (set SURSUM_SSH_PUBKEY)" >&2; exit 1; }
    mkdir -p target/image/work
    cp "$key" target/image/admin.pub
    system_version="$(git describe --always --dirty)"
    # The build host runs natively (rpi-image-gen emulates only the arm64 root file system), so
    # pin it to this machine's architecture: an arm64 base image pulled earlier must not win.
    podman build -q -t localhost/sursum-image-builder --platform "linux/$(podman info --format '{{{{.Host.Arch}}')" \
        --build-arg RIG_COMMIT="$(cut -d' ' -f2 image/rpi-image-gen.ref)" -f image/build.Containerfile image
    podman run --rm --privileged -v "$PWD:/src" localhost/sursum-image-builder \
        /opt/rpi-image-gen/rpi-image-gen build -S /src/image -c sursum.yaml -B /src/target/image/work -- \
        IGconf_sursum_bundle=/src/target/sursum-{{version}}.sursum-app \
        IGconf_ssh_pubkey_user1=/src/target/image/admin.pub \
        IGconf_sursum_diagnostic_boot={{diagnostic}} \
        IGconf_sursum_system_version="$system_version"
    out=target/image/work/image-sursum
    cargo run -q -p sursum-server -- system create --key "$SURSUM_BUNDLE_SIGNING_KEY" \
        --version "$system_version" --boot "$out/boot.vfat" --system "$out/system.erofs" \
        "target/sursum-system-$system_version.sursum-system"
    cargo run -q -p sursum-server -- system verify "target/sursum-system-$system_version.sursum-system"
    ls -lh "$out/sursum.img" "target/sursum-system-$system_version.sursum-system"

# The release goes on trial; the device goes back by itself if it does not run (ADR 0013).
# Install a bundle on a device over SSH (developers only, never during a service).
deploy host version: (app-bundle version)
    #!/usr/bin/env bash
    set -euo pipefail
    file=sursum-{{version}}.sursum-app
    scp target/$file {{host}}:/tmp/$file
    ssh {{host}} "sudo -u sursum /var/lib/sursum/current/sursum-server --data-dir /var/lib/sursum \
        bundle install /tmp/$file && rm /tmp/$file && sudo systemctl restart sursum"
    echo "waiting for the trial to end (30 s of healthy running)…"
    ssh {{host}} 'for i in $(seq 1 90); do test -e /var/lib/sursum/trial.json || break; sleep 1; done; \
        cat /var/lib/sursum/update.json; echo'

# Cross-compile the server for Windows (needs zig and cargo-zigbuild).
cross-win: build-web
    cargo zigbuild --release -p sursum-server --target x86_64-pc-windows-gnu
