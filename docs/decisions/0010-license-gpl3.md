# 0010 — GPL-3.0-or-later and dependency license policy

Status: Accepted (2026-11-15)

## Decision
The project is licensed under **GPL-3.0-or-later**:
- `LICENSE` contains the unmodified GPL-3.0 text from gnu.org;
- `license = "GPL-3.0-or-later"` in the Cargo workspace and every `package.json` (`"private": true`);
- SPDX header in every source file, checked in CI;
- the app shows license and no-warranty notice in "About", `sursum-server --license`, and a
  third-party license page at `/licenses` (`cargo about` + pnpm, including OFL fonts).

Dependencies must be GPL-3.0 compatible, enforced by `cargo deny`: MIT, Apache-2.0,
BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, OFL-1.1, LGPL-3.0, GPL-3.0(-or-later).
Not allowed: GPL-2.0-only, AGPL (without a new ADR), non-commercial licenses.

## Consequences
Code from other GPL projects (OpenSong, FreeShow) would be legally usable, but we keep writing our
own code; any exception needs an ADR. Song texts in parish databases are user data, not part of
the program. A future system image (M7) must provide sources or a written offer for its GPL
packages.
