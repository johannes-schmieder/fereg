#!/bin/bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
ROOT="$PWD"
PROBE=".ci/dev/codex-runner-probe.json"
MAX_PHASES="${STFIXEST_CODEX_PHASES:-8}"

if [[ ! -f "$PROBE" ]] || ! command -v codex >/dev/null 2>&1; then
  echo "No available Codex CLI probe; guarded loop is skipped."
  exit 0
fi

available="$(/usr/bin/python3 - <<'PY'
import json
from pathlib import Path
p = Path('.ci/dev/codex-runner-probe.json')
d = json.loads(p.read_text(encoding='utf-8'))
print('true' if d.get('codex_executable_available') and d.get('codex_exec_help_rc') == 0 else 'false')
PY
)"
if [[ "$available" != true ]]; then
  echo "Codex CLI probe is negative; guarded loop is skipped."
  exit 0
fi

source_sha() {
  (
    cd scripts/development
    /usr/bin/python3 - <<'PY'
import advance_stfeols as a
print(a.source_commit_from_head())
PY
  )
}

source_is_green() {
  local sha="$1"
  /usr/bin/python3 - "$sha" <<'PY'
import json
import sys
from pathlib import Path
sha = sys.argv[1]
path = Path('.ci/stata/results') / f'{sha}.json'
if not path.is_file():
    raise SystemExit(1)
d = json.loads(path.read_text(encoding='utf-8'))
ok = (
    d.get('tested_sha') == sha
    and d.get('status') == 'success'
    and d.get('stata_status') == 'success'
    and d.get('rust_status') == 'success'
    and d.get('stata_rc') == 0
    and d.get('rust_rc') == 0
    and d.get('tests_failed') == 0
)
raise SystemExit(0 if ok else 1)
PY
}

completion_is_bound() {
  local sha="$1"
  /usr/bin/python3 - "$sha" <<'PY'
import json
import sys
from pathlib import Path
path = Path('.ci/dev/stfeols-complete.json')
if not path.is_file():
    raise SystemExit(1)
d = json.loads(path.read_text(encoding='utf-8'))
required = {
    'source_sha': sys.argv[1],
    'public_activation': True,
    'full_scientific_qualification': 'success',
    'licensed_stata': 'success',
    'rust_source': 'success',
    'frozen_projection': 'success',
    'benchmark_gate': 'success',
    'public_transaction': 'success',
}
raise SystemExit(0 if all(d.get(k) == v for k, v in required.items()) else 1)
PY
}

help_text="$(codex exec --help 2>&1)"

for phase in $(seq 1 "$MAX_PHASES"); do
  git fetch origin main
  git checkout main
  git pull --ff-only origin main
  before="$(source_sha)"

  if completion_is_bound "$before" && source_is_green "$before"; then
    echo "stfeols completion marker is exact-source-bound and green at $before"
    exit 0
  fi
  if ! source_is_green "$before"; then
    echo "Current source $before is not exact-green; refusing another model phase."
    exit 1
  fi

  prompt_file="$(mktemp)"
  last_message="$(mktemp)"
  cat >"$prompt_file" <<PROMPT
You are phase $phase of a guarded continuation of private repository
johannes-schmieder/StFixest on its licensed Apple Silicon Stata/MP 18 runner.

Work directly on main; never create a branch. Inspect current main and recent
history yourself. Read AGENTS.md, STATUS.md, the active exact-SHA receipt, the
frozen OLS manifest and receipts, and all relevant source/tests before editing.
The current source is exact-green at $before. Complete exactly ONE coherent
next gate toward production-complete stfeols.

Mandatory policy:
- Preserve all scientific/sample/VCE/DoF/solver/certificate/transaction and
  fail-closed contracts. Never weaken tolerances to force parity.
- Pinned reghdfe comparison is necessary but does not replace independent
  mathematical/oracle checks.
- Public stfeols/estimate-ols remain disabled until every OLS 1.0 requirement,
  complete frozen projection, full licensed-Stata and Rust qualification,
  public eclass transaction/rollback, and scientific-first speed/RSS benchmark
  gate are genuinely complete.
- Use licensed Stata only on this runner. Commit no raw logs, local paths,
  license identity, secrets, or restricted data.
- Make small coherent commits on main, push, and bind acceptance to the exact
  tested source SHA. Update STATUS.md only from exact-green evidence.
- If the prior phase left a red or incomplete gate, repair that gate first.
  Otherwise follow STATUS.md's current order. Do not skip blockers.
- For ordinary implementation/debugging choices, use best judgment. Stop only
  at a genuinely scientific/API/architectural ambiguity requiring the user.

Run all relevant Rust, Python, C/loader, and licensed Stata tests for the gate.
Stop this invocation only when that one gate is exact-green and pushed, or at a
genuine ambiguity. Leave the checkout clean and main synchronized.

Only when stfeols is fully complete—including public activation and benchmark
acceptance—create .ci/dev/stfeols-complete.json with exactly these fields bound
to the accepted source SHA:
  source_sha, public_activation=true,
  full_scientific_qualification="success", licensed_stata="success",
  rust_source="success", frozen_projection="success",
  benchmark_gate="success", public_transaction="success".
Never create that marker for a partial or hidden-only checkpoint.
PROMPT

  args=(codex exec)
  if grep -q -- '--full-auto' <<<"$help_text"; then
    args+=(--full-auto)
  fi
  if grep -q -- '--cd' <<<"$help_text"; then
    args+=(--cd "$ROOT")
  elif grep -q -- '-C' <<<"$help_text"; then
    args+=(-C "$ROOT")
  fi
  if grep -q -- '--output-last-message' <<<"$help_text"; then
    args+=(--output-last-message "$last_message")
  fi
  if grep -q -- '--skip-git-repo-check' <<<"$help_text"; then
    args+=(--skip-git-repo-check)
  fi
  args+=("$(cat "$prompt_file")")

  set +e
  "${args[@]}"
  codex_rc=$?
  set -e
  rm -f "$prompt_file" "$last_message"
  if [[ "$codex_rc" -ne 0 ]]; then
    echo "Codex phase $phase failed with rc=$codex_rc"
    exit "$codex_rc"
  fi

  git fetch origin main
  git checkout main
  git pull --ff-only origin main
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "Codex phase $phase left an uncommitted worktree; stopping fail-closed."
    git status --short
    exit 1
  fi
  after="$(source_sha)"
  if [[ "$after" == "$before" ]]; then
    echo "Codex phase $phase made no source progress; stopping to avoid quota churn."
    exit 0
  fi
  if ! source_is_green "$after"; then
    echo "Codex phase $phase ended at non-green source $after; stopping fail-closed."
    exit 1
  fi
  echo "Codex phase $phase accepted exact-green source $after"
done

final="$(source_sha)"
if completion_is_bound "$final" && source_is_green "$final"; then
  echo "stfeols complete at exact-green source $final"
  exit 0
fi

echo "Guarded phase cap reached at exact-green source $final; more gates remain."
exit 0
