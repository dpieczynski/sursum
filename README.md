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

The dev environment is a devcontainer (Rust with the `wasm32-unknown-unknown` target, Trunk,
Chromium + chromedriver for WebDriver tests). No Node or npm. Setup instructions arrive with
milestone M0; see [docs/testing.md](docs/testing.md) and [CLAUDE.md](CLAUDE.md) for commands.

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
