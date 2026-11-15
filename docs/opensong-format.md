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

## To verify in M2 ⚠

Check against the OpenSong in-app help, the public OpenSong repository documentation and real
files (the parish library in `/local/`):

- `;` comment lines; other leading characters
- numbered lyric lines (`1`, `2`, … at line start inside `[V]`) expanding into V1, V2 with a shared
  chord line
- `|` single bar (line break?), `---`, `_`, `-`
- default order when `<presentation>` is empty; separators and case in `presentation`
- song references in sets: `name` + `path` attributes; `presentation` override on a set entry
- `image` slide groups (embedded image data?), other group types (`style`, external/video)
- `Songs/` subfolders, files without extension, BOM, CRLF, `&#13;` entities, legacy codepages
  (windows-1250 / ISO-8859-2 for Polish)

## Mapping to the internal model (OpenLyrics 0.9-compatible)

| OpenSong | sursum / OpenLyrics |
|---|---|
| `[V1]`, `[C]`, `[B]`, `[P]` | verse names `v1`, `c`, `b`, `p` |
| `[T]` | `e` (ending/tag) ⚠ confirm meaning |
| custom `[Name]` | `o<n>` with `label = "Name"` |
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
