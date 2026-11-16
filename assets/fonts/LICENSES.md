# Bundled fonts

All fonts are licensed under the SIL Open Font License 1.1 (OFL), which allows bundling them with
software. The full license texts are in this directory.

| Files | Font | Source | License |
|---|---|---|---|
| `lato-latin-{400,700}-normal.woff2`, `lato-latin-ext-{400,700}-normal.woff2` | Lato 2.0 by Łukasz Dziedzic | Fontsource `@fontsource/lato@5.3.0` (files fetched from the jsDelivr CDN, no npm tooling) | OFL-1.1, see `OFL-Lato.txt` (from google/fonts) |

The `latin-ext` subset covers all Polish letters (U+0100–017F). The `@font-face` rules with the
matching `unicode-range` values live in `crates/sursum-slide/style/slide.css`.
