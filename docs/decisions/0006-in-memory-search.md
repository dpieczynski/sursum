# 0006 — In-memory search with explicit Polish folding, not FTS5

Status: Accepted (2026-11-15)

## Context
Search by songbook number, title or lyric fragment must ignore Polish diacritics. `ł` (U+0142)
has no Unicode canonical decomposition, so NFD-based folding (and SQLite FTS5
`remove_diacritics`) does not turn it into `l`.

## Decision
`sursum-core::search` keeps an in-memory index of normalized text (NFKD, strip combining marks,
lowercase, explicit table `ł→l`, `đ→d`, `ø→o`, `ß→ss`, `æ→ae`, `œ→oe`, …) with simple ranking
(exact number > title prefix > word prefix > first line > phrase > all words).

## Consequences
Pure, testable code without FTS schema. A few thousand songs fit easily in memory; target
p95 < 30 ms on the Pi for 5000 songs, measured in M2. FTS5 stays an option if libraries grow.
