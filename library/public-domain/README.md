# Public-domain starter library

A starter library of Polish Catholic hymn texts (and a few Latin hymns) for sursum, made only of
texts that are in the public domain under Polish law. Every text is transcribed from a 19th-century
printed edition. Modern songbooks and websites were used only to find titles, never as a source of
text.

- `Songs/<Category>/<Title>`: one OpenSong song file per text (UTF-8 XML, no extension).
- `LICENSES.md`: the policy and one row per file (author or "traditional", source edition with
  URL of the scan or its Wikisource transcription, changes made).
- `REJECTED.md`: texts that were considered and left out, with the reason.

## License record in every file

Each song carries its license twice:

- in `<copyright>`, a short status such as
  `Domena publiczna – tekst anonimowy; wg wyd. Kraków 1838`;
- in hidden comment lines at the top of `<lyrics>` (lines starting with `;` are dropped on import
  and never shown): the source edition with page or number, the URL of the scan or Wikisource
  page, the author and death year (or why the text is anonymous), why it is public domain, and
  every change made (e.g. modernized spelling, omitted stanzas).

Old hymn numbers are deliberately left out (`<hymn_number>` is empty) so they cannot be confused
with a parish's own songbook numbering.

## Importing

In the remote: zip the `Songs` folder and use **Ustawienia → Import z OpenSong**.

From the command line:

```sh
cargo run -p sursum-server -- --data-dir <dir> import-opensong library/public-domain/Songs --songbook "Domena publiczna"
```

Re-importing updates the songs instead of duplicating them (song ids derive from the path inside
`Songs/`).

## Spelling

The texts keep the wording of the source edition. Only spelling is modernized (for example
`czem` → `czym`, `iej` → `jej`, `Xiężna` → `Księżna`, adjective endings `-emi` → `-ymi`); the
changes are listed in each file. Archaic word forms and grammar are kept as printed.
