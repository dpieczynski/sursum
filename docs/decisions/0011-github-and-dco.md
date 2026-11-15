# 0011 — GitHub hosting, DCO for contributions

Status: Accepted (2026-11-15)

## Decision
The repository is hosted at `github.com/dpieczynski/sursum` (default branch `main`); CI runs on
GitHub Actions. External contributions use the Developer Certificate of Origin (`git commit -s`)
rather than a CLA.

## Consequences
With DCO, contributors keep their copyright, so future relicensing or dual licensing would need
their consent. Accepted, since the project is GPL by choice. CI workflows avoid GitHub-only
features where practical, so moving to Forgejo Actions stays possible.
