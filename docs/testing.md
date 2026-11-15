# Testing

Commands become available during M0 (`justfile` is the source of truth).

| Command | Runs |
|---|---|
| `just test` | Rust unit + integration tests (cargo-nextest), Vitest |
| `just e2e` | Playwright against the built binary on a random port with a temporary data dir |
| `just lint` | rustfmt, clippy `-D warnings`, eslint, prettier, svelte-check, SPDX check, cargo-deny |

## Layers

- **Rust unit tests** (`sursum-core`, `sursum-formats`): OpenSong parser edge cases, slide
  compiler, program state machine (bounds, ghost position, live edits, overlay, `Stale` rule),
  Polish normalization and ranking, `content_region`, library merge. `proptest` for invariants
  (set operations, navigation bounds, layout never intersects a zone). `insta` JSON snapshots of
  protocol types.
- **Protocol integration tests** (`sursum-server/tests`, tokio-tungstenite): server on port 0
  with a `tempfile` data dir; hello/snapshot, commands, two remotes, rejected `Origin`, missing
  cookie, PIN rate limit, restart with the same/different `boot_id` (injected `Platform`), abort
  during a write.
- **Playwright** (`e2e/`): `chromium` project is blocking; `firefox` and `webkit` are
  non-blocking. Key tests: black first frame; editing the set never changes the screen; last
  slide stays on disconnect and reconnect has no transition; one font size per song; preview
  matches screen; black in one tap; hold-to-delete + undo; pedal key mapping; two remotes in sync;
  text never enters an exclusion zone.
- **On-device (M4)**: 100 power-cut cycles, late projector power-on and HDMI replug, 8-hour soak,
  HDMI capture of boot and failures.

## Fixtures

Only public-domain or synthetic texts. Add every new fixture to
[fixtures/LICENSES.md](../fixtures/LICENSES.md). OpenSong fixtures keep their exact bytes
(`.gitattributes` disables EOL conversion for them).

## Testing with a real parish library

Put a copy of the OpenSong folder under `/local/` (git-ignored, never committed), then:

```sh
SURSUM_REAL_LIBRARY=local/OpenSong cargo test -p sursum-formats -- --ignored
```

These tests assert statistics and absence of errors only; they never print or snapshot song text.
