#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python="${FEREG_PYTHON_BIN:-python3}"
"$python" -m unittest discover -s tests/python -p test_complexity.py
"$python" scripts/check_complexity.py
"$python" - <<'PY'
from pathlib import Path
import sys
violations = []
for crate in ['fereg-oracle']:
    root = Path('crates') / crate
    for path in sorted(root.rglob('*.rs')):
        if 'target' in path.parts:
            continue
        lines = len(path.read_text().splitlines())
        if lines > 1500:
            violations.append(f'{path}: {lines} lines exceeds 1500')
if violations:
    print('\n'.join(violations), file=sys.stderr)
    sys.exit(1)
print('oracle file size limits: PASS (functions checked by Clippy)')
PY
