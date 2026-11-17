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
| [0009](0009-display-engine-chromium.md) | Chromium under cage as the display engine (amended: policies now, a Rust WPEPlatform launcher once Debian ships WPE ≥ 2.54; cog rejected) |
| [0010](0010-license-gpl3.md) | GPL-3.0-or-later and dependency license policy |
| [0011](0011-github-and-dco.md) | GitHub hosting, DCO for contributions |
| [0012](0012-display-power-cec-pjlink.md) | Display power control via HDMI-CEC, PJLink fallback |
| [0013](0013-immutable-ab-image.md) | Immutable A/B device image (rpi-image-gen), signed app bundles |
| [0014](0014-rust-frontend-leptos.md) | Rust frontend: Leptos 0.8 CSR, Trunk, shared protocol/slide crates, WebDriver tests; no Dioxus |
| [0015](0015-screen-engine-portability.md) | Screen portability to WPE WebKit (cog): API contract, blocking WPE tests, engine switch in the image |
| [0016](0016-operator-and-admin-mode.md) | Operator rights by pairing, time-limited admin mode by entering the PIN again |
| [0017](0017-remote-three-views.md) | The remote: three views (Msza, Zestaw, Ustawienia) around a fixed live area |
| [0018](0018-whole-device-backup.md) | Whole-device backup (`.sursum-backup`) and restore from the remote, credentials stay on the device |
