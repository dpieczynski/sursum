# sursum-core — local rules

- **No I/O, no tokio, no SQLite, no clock reads.** Everything here is a pure function or a plain
  data type, so it can be unit-tested and reasoned about in isolation. Time and platform facts
  come in as parameters.
- Every public type and function has a `///` comment explaining why it exists.
- Newtypes for identifiers and counters (`SongId`, `ItemId`, `SlideKey`, `Rev`); enums instead of
  boolean flags.
- Every new parser/markup rule gets a synthetic fixture and a test. Fixtures must be listed in
  `fixtures/LICENSES.md`.
- Wire types live in `sursum-protocol`, not here. Core may use them (e.g. `Deck`, `Slide`) but
  must not add logic to them; protocol stays data-only so it compiles to wasm32.
- Prefer simple, readable algorithms over clever ones (e.g. `content_region` enumerates candidate
  rectangles; that is intentional).
- Invariants worth protecting get a `proptest` (set operations, navigation bounds, layout).
