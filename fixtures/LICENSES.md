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
