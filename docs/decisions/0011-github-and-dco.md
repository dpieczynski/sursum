# 0011 — GitHub hosting, DCO for contributions

Status: Accepted (2026-11-15); amended 2026-11-19

## Decision
The repository is hosted at `github.com/dpieczynski/sursum` (default branch `main`); CI runs on
GitHub Actions. External contributions use the Developer Certificate of Origin (`git commit -s`)
rather than a CLA.

## Consequences
With DCO, contributors keep their copyright, so future relicensing or dual licensing would need
their consent. Accepted, since the project is GPL by choice. CI workflows avoid GitHub-only
features where practical, so moving to Forgejo Actions stays possible.

## Amendment (2026-11-19): the repository becomes public
The repository was private while the first version took shape, and several things were worked
around because of it: device images and system updates were built only locally through QEMU
(no free arm64 runners), and free code signing for open-source projects was out of reach while
Windows Defender deletes the unsigned desktop program after download. The maintainer decided to
make it public now.

- Checked before publishing (2026-11-19): the whole history for keys, tokens, passwords and
  private addresses (none found; the addresses in it are test values), files that were ever
  committed and later removed (source files only), and the song texts (fixtures and the starter
  library are recorded as public domain or synthetic in their `LICENSES.md`; screenshots show
  the public-domain demo hymn). Commits carry the maintainer's name and e-mail address, as the
  DCO sign-off requires.
- Follows from it: image and system-update builds in CI on an arm64 runner (roadmap M4, M7,
  ADR 0022), the WPE package rebuild (ADR 0009 amendment), and an application for free signing
  of the Windows program (SignPath Foundation: its terms ask for an OSI license, no proprietary
  parts, builds from the repository by CI, and a published release) ⚠ not yet applied for.

## Amendment (2026-11-22): no SignPath signing
The SignPath Foundation declined the application for free signing of the Windows program. The
program stays unsigned (ADR 0020); the README's code signing policy section, which their terms
required, became a short note on signing and privacy.
