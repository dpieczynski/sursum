# sursum-core — local rules

- **No I/O, no tokio, no SQLite, no clock reads.** Everything here is a pure function or a plain
  data type, so it can be unit-tested and reasoned about in isolation. Time and platform facts
  come in as parameters.
- Every public type and function has a `///` comment explaining why it exists.
- Newtypes for identifiers and units (`SongId`, `ItemId`, `SlideKey`, `Px`); enums instead of
  boolean flags.
- Every new parser/markup rule gets a synthetic fixture and a test. Fixtures must be listed in
  `fixtures/LICENSES.md`.
- Protocol types derive `ts_rs::TS` behind the `ts` feature. Changing them means running
  `just bindings` and committing the regenerated TypeScript.
- Prefer simple, readable algorithms over clever ones (e.g. `content_region` enumerates candidate
  rectangles; that is intentional).
- Invariants worth protecting get a `proptest` (set operations, navigation bounds, layout).
