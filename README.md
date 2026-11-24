# sursum

Offline song-lyrics display for churches: a small appliance (Raspberry Pi 5 next to the projector)
shows lyrics to the congregation, and a one-hand remote in a tablet or PC browser controls it.

> **Status: pre-alpha.** Runs on a pilot Raspberry Pi 5 from its own device image (signed updates
> from the remote, with automatic rollback) and on a Linux PC with the screen and
> the remote in browsers; not yet used in a service. A desktop program (`sursum`, ADR 0020) lets
> parishioners prepare songs at home and bring them to the church as a file; its Windows build
> starts, the rest of the first run is still to be checked. Supported: Android tablets and
> Linux/Windows PCs as remotes, Windows and Linux for the desktop program (ADR 0023). See
> [docs/roadmap.md](docs/roadmap.md) and, for operators, [docs/operator-guide-pl.md](docs/operator-guide-pl.md).

## Goals

- The congregation only ever sees lyrics, black or a logo: no windows, cursors, errors or white flashes.
- What is on screen (the *program*) is separate from editing the set. Reordering or removing songs
  during a service never changes the screen until the operator moves on.
- The remote preview is the same rendering component as the screen.
- Works fully offline on a local network and survives sudden power loss.
- Imports songs and sets from OpenSong.

## Architecture in one paragraph

Everything is written in Rust. A single binary (`sursum-server`: axum, tokio, SQLite) holds all
state and logic. It serves two web apps written with Leptos (client-side rendering, compiled to
WebAssembly with Trunk) and embedded in the binary: the **screen** (Chromium kiosk under cage on
the Pi, read-only) and the **remote** (tablet or PC browser). Server and apps share the protocol
types (`sursum-protocol`) and the apps share the slide renderer (`sursum-slide`), so the remote
preview is the same component as the screen. Details: [docs/architecture.md](docs/architecture.md).

## Development

Requirements: rustup (the toolchain comes from `rust-toolchain.toml`), `just`, `trunk` 0.22,
`wasm-bindgen-cli` 0.2.130 (must match `Cargo.lock`), `leptosfmt`, `cargo-deny`, `cargo-about`,
zig with `cargo-zigbuild` (cross builds and the Windows lint), the GTK 4 and WebKitGTK 6.0
development packages (the desktop program's Linux window), and podman for the browser tests. The
exact versions are pinned in `.github/workflows/ci.yml`. No Node or npm.

```sh
just build                       # web apps (Trunk) + release server with the apps embedded
target/release/sursum-server --data-dir sursum-data --listen 127.0.0.1:8080
# screen: http://127.0.0.1:8080/screen/   remote: http://127.0.0.1:8080/remote/
just dev                         # server + both apps with auto-reload (:8080, :8081, :8082)
just desktop                     # the desktop program target/release/sursum (Linux: GTK 4, WebKitGTK 6.0 dev packages)
just test && just lint
just browsers-image && just browsers && just e2e
```

The demo set contains a public-domain hymn, a synthetic song and an announcement. A
`config.toml` in the data directory sets the listen address, allowed host names, the crossfade
duration, display power (`[display_power]`,
HDMI-CEC or PJLink) and the screen guard (`[screen_guard]`); see `crates/sursum-server/src/config.rs`.

Until a PIN is set nobody can control the screen: the first remote that opens the page sets it
(or run `sursum-server --data-dir … set-pin 123456`). A paired remote may do everything
(ADR 0024). The screen's margins and exclusion zones are set in the remote's calibration panel. `just dev` passes `--open-without-pin`: no PIN,
and the server only listens on loopback (it refuses any other address) — for development only.

## Download

Releases are built by GitHub Actions from the tagged commit and published on the
[releases page](https://github.com/dpieczynski/sursum/releases): the card image and the signed
system update for the Raspberry Pi 5, and the desktop program for Windows (unsigned, see
below). The card image lets nobody log in over SSH; to administer a device, add your
key on its persistent partition (ADR 0026). The free software inside the image and where its
sources are: [docs/sources.md](docs/sources.md).

## Signing and privacy

The Windows program (`sursum.exe`) is not code-signed: the application to the SignPath
Foundation's programme for open-source projects was declined (2026-11-22). Windows warns once
before running it; the operator guide shows how to get past the warning. Every release is built
by this repository's GitHub Actions workflows from the tagged source code.

The Raspberry Pi image and its system updates are signed with the project's own update key
(Ed25519, [docs/releasing.md](docs/releasing.md)); a device installs only updates signed with it.

Privacy: this program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it. The church device
serves its own pages and data only to the remotes on its local network, and the desktop program
listens on this computer only (loopback); neither sends telemetry or contacts the internet.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Never commit copyrighted song texts.

## License

Copyright (C) 2026 Dominik Pieczyński and contributors.

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
[GNU General Public License](LICENSE) for more details.

SPDX-License-Identifier: `GPL-3.0-or-later`. Third-party licenses are listed in the app under
`/licenses`.
