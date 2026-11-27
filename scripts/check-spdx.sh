#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Fails if a source file lacks the SPDX license header (ADR 0010).
set -euo pipefail
cd "$(dirname "$0")/.."
missing=0
while IFS= read -r file; do
  if ! head -n 3 "$file" | grep -q "SPDX-License-Identifier: GPL-3.0-or-later"; then
    echo "missing SPDX header: $file"
    missing=1
  fi
done < <(git ls-files --cached --others --exclude-standard \
  '*.rs' '*.css' '*.html' '*.sh' '*.ps1' '*.sql' '*.toml' 'justfile' '*Containerfile' '*.yml' \
  '*.java' '*.kts' '*.properties' 'android/*.xml' '*.service' \
  | grep -v -E '^(Cargo\.lock|fixtures/)')
exit "$missing"
