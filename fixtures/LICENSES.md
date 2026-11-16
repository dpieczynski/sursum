# Fixture sources and licenses

Every file under `fixtures/` must be listed here with its source and status. Allowed statuses:

- **public-domain**: text (and, where relevant, melody) whose author died more than 70 years ago,
  taken from a public-domain source (e.g. Wikisource), with the source URL;
- **synthetic**: text written for this project to exercise a parser rule; no real song.

Copyrighted texts are never allowed, including excerpts. Real parish libraries are tested locally
from `/local/` (git-ignored), see [docs/testing.md](../docs/testing.md).

| Path | Status | Source / notes |
|---|---|---|
| `demo/kiedy-ranne-wstaja-zorze.json` | public-domain | Franciszek Karpiński (1741–1825), "Pieśń poranna" from *Pieśni nabożne* (1798); checked against <https://pl.wikisource.org/wiki/Pie%C5%9B%C5%84_poranna_(Karpi%C5%84ski,_1919)>, spelling modernized (e.g. "Twymi" for "Twemi", "wczoraj" for "wczora"). |
| `demo/synthetic-refrain.json` | synthetic | Written for sursum to exercise refrains, presentation order, optional and forced breaks and Polish diacritics. |
| `opensong/Songs/Synthetic Basic` | synthetic | UTF-8, chords, comment, trailing `|`, `||`, presentation with repeats. |
| `opensong/Songs/Kolędy/Synthetic Numbered` | synthetic | UTF-8 with BOM and CRLF, numbered verse lines, Polish `[R]` refrain, digit presentation tokens, folder with Polish letters. |
| `opensong/Songs/Synthetic CP1250` | synthetic | Declared windows-1250 encoding, every Polish letter, empty presentation. |
| `opensong/Songs/Synthetic Broken` | synthetic | Malformed XML (must be reported, not crash). |
| `opensong/Songs/cover.jpg` | synthetic | Not an image at all; skipped by extension. |
| `opensong/Sets/Synthetic Sunday` | synthetic | Set with song entries (with and without `path`, with a `presentation` override, one missing), a custom announcement, a scripture group with invented text and an image group (skipped with a warning). |
