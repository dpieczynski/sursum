# Architecture Decision Records

Short records of decisions that are expensive to revisit. Format: Context, Decision,
Consequences. Status is `Accepted` unless superseded; to change a decision, add a new ADR that
supersedes the old one instead of editing it.

| ADR | Decision |
|---|---|
| [0001](0001-pi-next-to-screen.md) | Raspberry Pi 5 next to the screen, Ethernet, optional PoE |
| [0002](0002-no-midi.md) | No MIDI support |
| [0003](0003-restart-policy.md) | Logo after OS reboot, full restore after process restart |
| [0004](0004-tap-goes-live.md) | Tapping a slide in the remote goes live immediately |
| [0005](0005-snapshots-and-frozen-program.md) | Snapshot protocol, frozen program, `Stale` rule |
| [0006](0006-in-memory-search.md) | In-memory search with explicit Polish folding, not FTS5 |
| [0007](0007-http-first.md) | Plain HTTP in the MVP; optional device CA later |
| [0008](0008-exclusion-zones.md) | Screen profile with exclusion zones (pillar in Tulce) |
| [0009](0009-display-engine-chromium.md) | Chromium under cage as the display engine for now (amended: policies; cog and WebKitGTK under cage rejected; a WPE-on-DRM spike with a rebuilt 2.54 package and WebKit's own MiniBrowser decides the switch) |
| [0010](0010-license-gpl3.md) | GPL-3.0-or-later and dependency license policy |
| [0011](0011-github-and-dco.md) | GitHub hosting, DCO for contributions; public repository (amended 2026-11-19) |
| [0012](0012-display-power-cec-pjlink.md) | Display power control via HDMI-CEC, PJLink fallback (both built) |
| [0013](0013-immutable-ab-image.md) | Immutable A/B device image (rpi-image-gen), the image stands; its signed app bundles are superseded by ADR 0022 |
| [0014](0014-rust-frontend-leptos.md) | Rust frontend: Leptos 0.8 CSR, Trunk, shared protocol/slide crates, WebDriver tests; no Dioxus |
| [0015](0015-screen-engine-portability.md) | Screen portability to WPE WebKit: API contract and blocking WPE tests (still started through cog; their future follows the WPE spike); the cog image variant was withdrawn |
| [0016](0016-operator-and-admin-mode.md) | Rights by pairing with the PIN, revocation, hardening (its admin mode is superseded by ADR 0024) |
| [0017](0017-remote-three-views.md) | The remote: three views (Msza, Zestaw, Ustawienia) around a fixed live area |
| [0018](0018-whole-device-backup.md) | Whole-device backup (`.sursum-backup`) and restore from the remote, credentials stay on the device |
| [0019](0019-offline-system-updates.md) | Offline system updates: signed `.sursum-system` from the remote, written to the other A/B slot, trial through the firmware's tryboot; the only update path once ADR 0022 is built |
| [0020](0020-desktop-program.md) | Desktop program `sursum`: the same server on loopback in a window of its own (WebView2 on Windows, WebKitGTK 6.0 on Linux); the window has no features; Windows supported, Linux best effort |
| [0021](0021-remote-on-webkit.md) | The remote on WebKit: ADR 0015's API contract extended to the remote, blocking remote tests on WebKitGTK (the iPad parts are superseded by ADR 0023) |
| [0022](0022-one-update-path.md) | One update path: the server ships inside the system image, `.sursum-system` is the only update file, application bundles are removed (not built yet) |
| [0023](0023-platform-scope.md) | Platform scope: no iPad, no macOS program, no Windows parish PC; remote in Blink browsers, desktop program on Windows and Linux, screen on the Pi image only |
| [0024](0024-one-level-of-rights.md) | One level of rights: a paired remote may do everything; no admin mode, pairing stays |
