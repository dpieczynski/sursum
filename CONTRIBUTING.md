# Contributing to sursum

Thank you for your interest! The project is in an early stage; please open an issue before
starting larger work.

## License and sign-off (DCO)

sursum is licensed under **GPL-3.0-or-later**. By contributing, you agree that your contribution
is licensed under the same terms.

Every commit must be signed off under the
[Developer Certificate of Origin 1.1](https://developercertificate.org/):

```sh
git commit -s -m "Describe the change"
```

This adds a `Signed-off-by: Your Name <you@example.com>` line, certifying that you wrote the
change or otherwise have the right to submit it under the project license.

## Hard rules

- **No copyrighted song texts** anywhere in the repository: not in fixtures, tests, docs, issues
  or screenshots. Use public-domain texts or synthetic text, and record the source of every
  fixture in [fixtures/LICENSES.md](fixtures/LICENSES.md).
- **No code copied from other projects** (e.g. OpenSong, FreeShow) unless its license is
  compatible and the decision is recorded in an ADR in [docs/decisions/](docs/decisions/).
  Reading documentation to understand file formats or behavior is fine.
- New dependencies must use a license compatible with GPL-3.0 (enforced by `cargo deny`).
- Every source file starts with an SPDX header, e.g. `// SPDX-License-Identifier: GPL-3.0-or-later`.

## Conventions

- Code, comments, commit messages and technical docs: **English**. User interface: **Polish**,
  through the i18n layer (never hard-coded strings).
- Readable code over clever code. See the principles in [CLAUDE.md](CLAUDE.md).
- A change is done when tests, `clippy`, `svelte-check` and lints pass, generated protocol types
  are up to date, and [docs/roadmap.md](docs/roadmap.md) reflects the progress.
