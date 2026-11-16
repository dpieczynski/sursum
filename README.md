# sursum

Offline song-lyrics display for churches: a small appliance (Raspberry Pi 5 next to the projector)
shows lyrics to the congregation, and a one-hand remote in a tablet or PC browser controls it.

> **Status: pre-alpha.** Nothing is usable yet. See [docs/roadmap.md](docs/roadmap.md).

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

Requirements: rustup (the toolchain comes from `rust-toolchain.toml`), `trunk` 0.22,
`wasm-bindgen-cli` 0.2.130 (must match `Cargo.lock`), `just`, and podman for the browser tests.
The devcontainer in `.devcontainer/` has all of them. No Node or npm.

```sh
just build                       # web apps (Trunk) + release server with the apps embedded
target/release/sursum-server --data-dir sursum-data --listen 127.0.0.1:8080
# screen: http://127.0.0.1:8080/screen/   remote: http://127.0.0.1:8080/remote/
just dev                         # server + both apps with auto-reload (:8080, :8081, :8082)
just test && just lint
just browsers-image && just browsers && just e2e
```

The demo set contains a public-domain hymn, a synthetic song and an announcement. A
`config.toml` in the data directory sets the listen address, the crossfade duration and the screen
profile (safe area, exclusion zones); see `crates/sursum-server/src/config.rs`.

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
`/licenses` (from milestone M0).
