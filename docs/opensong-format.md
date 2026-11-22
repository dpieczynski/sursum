# OpenSong formats — what we know

Working notes for the importer (`crates/sursum-formats`). We read documentation and observe
behavior; we do not copy OpenSong code (GPL, ADR 0010 policy).

## Verified (sources read 2026-11-15)

From <https://opensong.org/development/file-formats/>:

- **Song**: XML root `<song>` with `title`, `author`, `copyright`, `presentation`, `hymn_number`,
  `capo` (attribute `print`), `tempo`, `timesig`, `ccli`, `theme`, `alttheme`, `user1`–`user3`,
  `key`, `aka`, `key_line`, `lyrics`, `style` (attribute `index`). Note the spec says `timesig`;
  accept `time_sig` too.
- **Set**: `<set>` → `<slide_groups>` → `<slide_group name type …>`. Types in the spec: `custom`
  (attributes `seconds`, `loop`; children `title`, `subtitle`, `notes`, `slides/slide/body`),
  `scripture` (`title`, `subtitle`, `slides/slide/body`, `notes`), `song` (empty element).
- **Encoding**: any W3C encoding may be declared; ISO-8859-1 is common in western files, UTF-8 is
  recommended.

From <https://opensong.org/documentation/getting-started/>:

- Lyric lines must begin with at least one **space**; chord lines begin with a **period** `.`.
- Section headers in square brackets: `V` verse, `1`–`9` numbered verses, `C` chorus, `B` bridge,
  `P` pre-chorus, `T` tag.
- Each section starts a new slide; `||` forces a slide break inside a section.
- Song title is also the file name.

## Observed in a real Polish library (2026-11-16)

Statistics over 875 songs of a public Polish OpenSong songbook (markup only; no text was copied):
all UTF-8 without BOM; the element is `time_sig` (the spec says `timesig`; both are accepted);
`linked_songs` and `backgrounds` appear; headers `[V1]`, `[C]`, `[C2]`, lower-case `[v1]`,
suffixed `[V1A]`, and `[R1]`/`[R2]` (Polish *refren*) referenced as `C1`/`C2` in the presentation;
presentation tokens `1` meaning `V1`; a single `|` at the end of lines; occasional stray `.` or `/`
lines; typos in presentation orders (`CV2`, `V78`). The importer now handles all of these (typos are
reported). Result: 875 songs, 0 failures, 6291 slides, 9 warnings (all genuine typos).

A local OpenSong 3.4.8 installation (2026-11-16, structure only): the set root `<set>` had **no
`name` attribute**; song groups had only `name` and `type` (**no `path`, no `presentation`**);
`custom` groups carry `seconds`, `loop`, `title`, `subtitle`, `notes`, `slides/slide/body`;
`scripture` groups appeared. All four song entries matched a library file by name.

## Implemented rules (`crates/sursum-formats/src/opensong/`)

- Section headers map to OpenLyrics names: `V1`→`v1`, `C`→`c`, **`R`→`c`** (Polish refrain),
  `B`→`b`, `P`→`p`, `T`/`E`→`e`, `I`→`i`, `1`…`9`→`v1`…`v9`, `V1A`→`v1a`; anything else → `o<n>`
  with the header as label. Case-insensitive.
- Presentation tokens resolve case-insensitively; `C1` ≡ `C` and `V1` ≡ `V`; unknown → warning.
- `;` comments are dropped; `.` lines are chords attached to the next lyric line (positions kept,
  not displayed); a line starting with another character is kept with a warning.
- `||` forces a new slide; `|` breaks the line; empty fragments are dropped (so a trailing `|`
  adds nothing).
- Numbered lines (`1 …`, `2 …`) inside a section become verses `<kind>1`, `<kind>2`, sharing the
  chord line above them. ⚠ Inferred from OpenSong usage; not seen in the sample library.
- Duplicate section headers are merged with a slide break and a warning (custom ones too).
- An element that should appear once but appears twice with different text: the first is
  used, with a warning.
- Encoding: BOM, then the declared encoding, then UTF-8, then windows-1250 (with a warning).
  Bytes invalid in the file's encoding become U+FFFD, with a warning.
- Song ids derive from the path inside `Songs/`, so re-importing updates instead of duplicating.
- **Sets** (`Sets/<file>`, read from the folder next to `Songs/` or from the uploaded ZIP): the set
  name is the `name` attribute, else the file name. Song groups are found in the library by
  `path` + `name` when `path` exists, else by file name (shortest path wins, reported when
  ambiguous), else by title (songs typed in sursum). A `presentation` attribute becomes the item's
  verse order (same token rules as songs). `custom` and `scripture` groups become text items (one
  slide per `<body>`). `image` and other groups are skipped with a report line; missing songs are
  skipped and reported. Set and item ids derive from the file name, so re-importing a set replaces
  it and keeps item ids.

## Still to verify ⚠

- `_` and `-` inside lyric lines (not seen in the sample; kept as text);
- set files: `image` slide groups (structure guessed in the fixture), `presentation` override and
  `path` on song entries (not seen locally), `style` groups;
- legacy encodings in real parish files.

## Mapping to the internal model (OpenLyrics 0.9-compatible)

| OpenSong | sursum / OpenLyrics |
|---|---|
| `[V1]`, `[C]`, `[B]`, `[P]` | verse names `v1`, `c`, `b`, `p` |
| `[T]` | `e` (ending/tag) |
| `[R]`, `[R1]` | `c`, `c1` (Polish *refren*) |
| custom `[Name]` | `o<n>` with `label = "Name"` (numbers taken by `[O<n>]` are skipped) |
| `||` | new `LinesBlock` with `Forced` break (export: separate `<lines>`) |
| chord line `.` | chords attached to the next lyric line (stored, not displayed in MVP) |
| `presentation` | `verse_order` |
| `hymn_number` | `songbooks[].entry` |
| `aka` | additional title |
| `user1`–`3`, `key`, `capo`, `tempo`, `timesig`, `key_line` | `extra` map (lossless) |

OpenLyrics 0.9 has `<lines break="optional">` for optional splits
(<https://docs.openlyrics.org/en/latest/dataformat.html>); forced breaks have no direct equivalent,
so they are exported as separate `<lines>` blocks.

## Edge cases to cover with synthetic fixtures

Missing `presentation`; reference to a missing section; duplicate sections; numbered lines with
gaps; chord-only lines; empty sections; `||` at start, end and twice; CRLF and BOM; unknown
elements; declared windows-1250; set referencing a missing song; sets with custom, scripture and
image groups.

## The song editor (not the import)

The remote's editor accepts more than OpenSong, so nobody has to learn the markup first
(`sursum_formats::opensong::editor_markup`, tests in `editor_text.rs`):

- **Plain text** (no `[section]` line): blocks separated by an empty line are verses; a block
  starting with "Ref.", "Ref:", "Ref", "Refren", "R." or "R:" is the refrain, and a block that is
  only such a marker repeats it there (the presentation order is then generated, unless one is
  typed under "Zaawansowane"). "Most:" or "Bridge" starts a bridge. A leading verse number
  ("1. ", "2) ") is dropped.
- **Markup**: as above, but lyric lines may omit the leading space; it is added before parsing.
  Songs without chords are shown in the editor without those spaces.

The import keeps the strict OpenSong rules and reports lines without the leading space.
