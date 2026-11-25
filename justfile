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

# Build the desktop program `sursum` (ADR 0020) for this computer (Linux needs the GTK 4 and
# WebKitGTK 6.0 development packages; Windows builds it with MSVC, see the CI).
desktop: build-web
    cargo build --release -p sursum-desktop

# The desktop program for Windows as one file (ADR 0020): MSVC links the WebView2 loader in, so
# it is built on Windows (CI and releases). The web apps and the program share the build id above.
desktop-windows: build-web
    cargo build --release -p sursum-desktop --target x86_64-pc-windows-msvc

# The server's tests on Windows, where the desktop program runs it (after `desktop-windows`,
# which builds the embedded apps). Catches what only Windows does, e.g. with directories.
test-windows:
    cargo test -p sursum-core -p sursum-formats -p sursum-server --target x86_64-pc-windows-msvc

# Install the desktop program for this Linux user: the binary in ~/.local/bin, the menu entry and
# its icon (GTK finds the window's icon by the application id).
desktop-install: desktop
    install -Dm755 target/release/sursum ~/.local/bin/sursum
    install -Dm644 crates/sursum-desktop/assets/io.github.dpieczynski.Sursum.desktop ~/.local/share/applications/io.github.dpieczynski.Sursum.desktop
    install -Dm644 crates/sursum-desktop/assets/sursum.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.dpieczynski.Sursum.svg

# Start the desktop program with an empty data folder until the remote in its window connects
# (needs a display; CI runs it under `dbus-run-session -- xvfb-run -a`).
desktop-smoke: desktop
    scripts/desktop-smoke.sh target/release/sursum

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

# Build the images with browsers and WebDriver servers for the end-to-end tests: Chromium and
# WebKitGTK on trixie, WPE WebKit 2.54 with MiniBrowser on forky (the device's engine, ADR 0027).
browsers-image:
    podman build -t localhost/sursum-browsers -f e2e/browsers.Containerfile e2e
    podman build -t localhost/sursum-wpe -f e2e/wpe.Containerfile e2e

# Start chromedriver (:9515), WPEWebDriver (:9516) and WebKitGTK's WebDriver (:9517) in the
# background, on localhost only.
# `--init` reaps finished browser processes; without it they pile up as zombies until new
# sessions fail ("Timed out receiving message from renderer").
browsers:
    podman run -d --rm --init --name sursum-chromedriver --network host localhost/sursum-browsers chromedriver --port=9515
    podman run -d --rm --init --name sursum-wpedriver --network host localhost/sursum-wpe WPEWebDriver --port=9516
    podman run -d --rm --init --name sursum-webkitgtkdriver --network host localhost/sursum-browsers webkitgtk-webdriver 9517

# Stop the WebDriver containers.
browsers-stop:
    -podman stop sursum-chromedriver sursum-wpedriver sursum-webkitgtkdriver

# Browser tests of the font fitting in the shared drawing crate (needs `just browsers`).
test-wasm:
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner CHROMEDRIVER_REMOTE=http://127.0.0.1:9515 \
        cargo test -p sursum-slide --target wasm32-unknown-unknown --test dom

# End-to-end tests: everything on Chromium, then the screen on WPE WebKit (MiniBrowser, ADR 0015), then
# the remote on WebKitGTK with the screen on Chromium (ADR 0021: the Linux window is WebKit).
e2e: build-web
    cargo build -p sursum-server --features e2e-hooks
    SURSUM_E2E_BROWSER=chromium cargo test -p sursum-e2e -- --ignored --test-threads=1
    SURSUM_E2E_BROWSER=wpe cargo test -p sursum-e2e -- --ignored --test-threads=1
    SURSUM_E2E_BROWSER=chromium SURSUM_E2E_REMOTE_BROWSER=webkitgtk cargo test -p sursum-e2e \
        --test remote --test remote_live --test remote_sets --test remote_library -- --ignored --test-threads=1

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
    cargo-zigbuild clippy -p sursum-desktop --target x86_64-pc-windows-gnu --all-targets -- -D warnings
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

# The update key must be there before the Pi build: its public half is compiled into the server
# (a device accepts only updates signed with it), the secret signs the system update.
update-keys:
    @test -n "${SURSUM_BUNDLE_KEY:-}" || { echo "set SURSUM_BUNDLE_KEY (public update key, see docs/releasing.md)" >&2; exit 1; }
    @test -f "${SURSUM_BUNDLE_SIGNING_KEY:-}" || { echo "set SURSUM_BUNDLE_SIGNING_KEY (path of the secret key file)" >&2; exit 1; }

# The rebuilt WPE WebKit packages the image installs (ADR 0027): the `wpewebkit-debs` artifact of a
# successful run of .github/workflows/wpewebkit.yml, into target/wpe-debs. By default the latest
# run on main started by a push or by hand: a pull request's run (a fork's branch may be called
# main too) must never reach a release, which signs whatever this fetches. `just wpe-debs any`
# takes the latest successful run of any branch (trial images, which a throwaway key signs);
# `just wpe-debs <run id>` takes that run.
wpe-debs from="main":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{from}}" in
        main) pick='select(.headBranch == "main" and (.event == "push" or .event == "workflow_dispatch"))' ;;
        any) pick='.' ;;
        *[!0-9]* | "") echo "wpe-debs: main, any or a run id, not '{{from}}'" >&2; exit 1 ;;
        *) pick="select(.databaseId == {{from}})" ;;
    esac
    run=$(gh run list --workflow wpewebkit.yml --status success --limit 100 \
        --json databaseId,headBranch,event --jq "[.[] | $pick][0].databaseId // empty")
    test -n "$run" || { echo "no successful wpewebkit.yml run for '{{from}}'" >&2; exit 1; }
    rm -rf target/wpe-debs
    gh run download "$run" --name wpewebkit-debs --dir target/wpe-debs
    ls target/wpe-debs

# Device image for a Raspberry Pi 5 (ADR 0013, image/) with this checkout's server in its
# read-only system (ADR 0022) and the WPE WebKit screen (ADR 0027; the packages from
# `just wpe-debs` in target/wpe-debs); `just image y` makes a diagnostic image (boot messages
# instead of black).
# Also writes the matching system update target/sursum-system-<version>.sursum-system,
# the only way devices are updated. The version is `git describe`. Needs the update keys (see
# update-keys) and the administrator's SSH public key (SURSUM_SSH_PUBKEY: a file, default
# ~/.ssh/id_ed25519.pub, or the key itself as in CI; never kept in the repository). Builds with
# the pinned rpi-image-gen in a privileged podman container (SURSUM_PODMAN="sudo podman" where
# rootless podman cannot, as in CI); natively on arm64, on x86_64 through QEMU (slow).
image diagnostic="n": update-keys cross-pi
    #!/usr/bin/env bash
    set -euo pipefail
    # The administrator's key goes to /home on the persistent partition (kept by system
    # updates); `none` builds an image nobody can log in to (public releases, ADR 0026).
    key="${SURSUM_SSH_PUBKEY:-$HOME/.ssh/id_ed25519.pub}"
    mkdir -p target/image/work
    case "$key" in
        none) : > target/image/admin.pub ;;
        ssh-*) printf '%s\n' "$key" > target/image/admin.pub ;;
        *) test -f "$key" || { echo "no SSH public key at $key (set SURSUM_SSH_PUBKEY, or none)" >&2; exit 1; }
           cp "$key" target/image/admin.pub ;;
    esac
    system_version="$(git describe --always --dirty)"
    ls target/wpe-debs/libwpewebkit-2.0-1_*.deb >/dev/null \
        || { echo "no rebuilt WPE WebKit packages in target/wpe-debs (just wpe-debs)" >&2; exit 1; }
    podman="${SURSUM_PODMAN:-podman}"
    # The build host runs natively (rpi-image-gen emulates only the arm64 root file system), so
    # pin it to this machine's architecture: an arm64 base image pulled earlier must not win.
    $podman build -q -t localhost/sursum-image-builder --platform "linux/$($podman info --format '{{{{.Host.Arch}}')" \
        --build-arg RIG_COMMIT="$(cut -d' ' -f2 image/rpi-image-gen.ref)" -f image/build.Containerfile image
    $podman run --rm --privileged -v "$PWD:/src" localhost/sursum-image-builder \
        /opt/rpi-image-gen/rpi-image-gen build -S /src/image -c sursum.yaml -B /src/target/image/work -- \
        IGconf_sursum_server=/src/target/aarch64-unknown-linux-gnu/release/sursum-server \
        IGconf_ssh_pubkey_user1=/src/target/image/admin.pub \
        IGconf_sursum_diagnostic_boot={{diagnostic}} \
        IGconf_sursum_system_version="$system_version" IGconf_sursum_wpe_debs=/src/target/wpe-debs
    out=target/image/work/image-sursum
    cargo run -q -p sursum-server -- system create --key "$SURSUM_BUNDLE_SIGNING_KEY" \
        --version "$system_version" --boot "$out/boot.vfat" --system "$out/system.erofs" \
        "target/sursum-system-$system_version.sursum-system"
    cargo run -q -p sursum-server -- system verify "target/sursum-system-$system_version.sursum-system"
    ls -lh "$out/sursum.img" "target/sursum-system-$system_version.sursum-system"

# Run this checkout's server on a device until it restarts (developers only, never before a
# service; ADR 0022): copies the Pi build to /run (RAM) and points sursum.service at it with a
# runtime drop-in. Any reboot brings back the system's own server. Careful: a build that raises
# the database schema leaves a database the system's server refuses after that reboot; put back
# a copy from before the try (backups/) or install a system update with the new server.
try host: cross-pi
    #!/usr/bin/env bash
    set -euo pipefail
    scp target/aarch64-unknown-linux-gnu/release/sursum-server {{host}}:/tmp/sursum-server-try
    ssh {{host}} 'set -eu
        sudo install -D -m 0755 /tmp/sursum-server-try /run/sursum-try/sursum-server
        rm /tmp/sursum-server-try
        sudo mkdir -p /run/systemd/system/sursum.service.d
        printf "[Service]\nExecStart=\nExecStart=/run/sursum-try/sursum-server --data-dir /var/lib/sursum --listen 0.0.0.0:80\n" \
            | sudo tee /run/systemd/system/sursum.service.d/try.conf >/dev/null
        sudo systemctl daemon-reload
        sudo systemctl restart sursum'
    echo "{{host}} runs this checkout's server ($SURSUM_BUILD_ID) until it restarts"
