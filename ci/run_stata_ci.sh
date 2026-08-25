#!/bin/bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
python_bin="${FEREG_PYTHON_BIN:-}"
if [[ -z "$python_bin" ]]; then
  python_bin="$(command -v python3 || true)"
fi
if [[ -z "$python_bin" ]]; then
  printf '%s\n' 'set FEREG_PYTHON_BIN or place python3 on PATH' >&2
  exit 127
fi
exec "$python_bin" "$script_dir/run_stata_ci.py" "${1:-smoke}"
