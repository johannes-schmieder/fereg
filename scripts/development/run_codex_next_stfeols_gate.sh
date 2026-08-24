#!/bin/bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
ROOT="$PWD"
PROBE=".ci/dev/codex-runner-probe.json"

if [[ ! -f "$PROBE" ]]; then
  echo "No coding-agent probe is available; bounded agent gate is skipped."
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
  echo "Codex CLI is not available and usable on this runner; bounded agent gate is skipped."
  exit 0
fi

help_text="$(codex exec --help 2>&1)"
prompt_file="$(mktemp)"
last_message="$(mktemp)"
cleanup() {
  rm -f "$prompt_file" "$last_message"
}
trap cleanup EXIT

cat >"$prompt_file" <<'PROMPT'
You are continuing active development of the private repository
johannes-schmieder/StFixest on its licensed Apple Silicon Stata/MP 18 runner.

Work directly on main. Do not create branches. Inspect current main, recent
history, AGENTS.md, STATUS.md, SCIENTIFIC_QUESTION.md, the frozen OLS manifest,
exact-SHA receipts, and current workflow state before editing. Public stfeols,
stfepois, estimate-ols, and estimate-ppml must remain fail-closed unless every
governing scientific, transaction, parity, performance, and activation gate is
actually complete and exact-green.

Johannes has now made the explicit scientific decision: OPTION A. This decision
supersedes the current text of SCIENTIFIC_QUESTION.md and the recently accepted
signed-residual-DoF compatibility route in commits c8b2a884159b6c9941c40b27e033437f665442b2,
6bbe04d857a293f0fa1fb0f617683be9ad5b4ea1, and 1fba97899ce7fd13a54307f764ea47e0e3b36442.
Do not advance to slope_only_multiple or any later blocker until this correction
is exact-green and source-bound.

Your task in this invocation is ONE coherent corrective gate implementing option
A completely:

1. Preserve the useful absorbed intercept-plus-one-slope engineering already
   added: guarded request planning, final-sample slope-source binding, compact
   topology ownership, allocation-bounded certified MAP, solver certificates,
   transaction atomicity, and independent explicit-dummy numerical oracles.
2. Remove or revert the isolated signed residual-DoF/statistics/covariance
   publication path and every acceptance of negative iid variance. The ordinary
   positive-residual-DoF OLS inference contract must again be the only eligible
   inference contract. Nonpositive residual DoF must fail closed before any
   result publication or logical transaction commit.
3. Repair the frozen `intercept_and_slope` semantic test design by adding a
   dedicated deterministic synthetic absorbed-slope fixture with enough
   observations and support for strictly positive residual DoF. Keep the case
   id and command shape exactly:

       reghdfe y c.x1, absorb(firm##c.time worker)

   Preserve the absorbed intercept/slope identities and omission/status
   semantics. Update the frozen manifest's dataset binding and all source hashes
   deliberately and transparently. Do not alter unrelated semantic cases.
4. Recapture and reseal the pinned licensed-reghdfe reference evidence for the
   repaired positive-DoF case using the exact offline dependency closure. Bind
   the hidden StFixest case to the new positive-DoF sample, coefficients/status,
   absorbed DoF metadata, positive df_r, nonnegative iid covariance, fit
   statistics, residual/sample identity, and mandatory native certificates.
5. Retain a separate independent over-absorption failure test based on the old
   24-row design (or an equivalent explicit construction) proving that
   nonpositive residual DoF returns the registered insufficient-observations
   failure, posts no covariance/statistics, leaves private completion false, and
   preserves prior Stata e() state / requested output atomicity where applicable.
   The historical reghdfe negative-DoF result may remain documented as the
   reason for the repaired fixture, but it must not remain an executable
   StFixest success contract.
6. Correct SCIENTIFIC_QUESTION.md to record option A as Johannes's decision.
   Update STATUS.md only after exact-green evidence exists: identify the accepted
   source and receipts, state that signed-DoF publication was removed, describe
   the positive-DoF fixture and failure oracle, and name the true next blocker.

Treat reghdfe comparison as necessary but not sufficient. Preserve independent
explicit-dummy rank/residual/RSS/covariance checks, allocation-failure coverage,
solver-certificate mutation tests, and failure atomicity. Do not weaken
numerical tolerances, sample rules, VCE behavior, or certificate requirements.
Run all relevant Rust, Python, C/loader, licensed-Stata differential, and full
qualification tests on this runner. Only call a source accepted when exact-SHA
receipts record overall, Rust, and licensed Stata success. Make small coherent
commits directly to main and push after meaningful changes; do not commit raw
licensed logs, local paths, secrets, or restricted data.

Stop after this one corrective option-A gate is exact-green, resealed, pushed,
and documented, or at the first genuinely new scientific/API/architectural
ambiguity requiring Johannes. For ordinary implementation and debugging choices,
use best judgment and continue. Leave a concise final report naming source SHA,
reference/candidate/full receipt paths, tests, principal files changed, and the
exact next gate.
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

"${args[@]}"

git fetch origin main
git checkout main
git pull --ff-only origin main

# Record only bounded metadata, never the agent transcript or private logs.
mkdir -p .ci/dev
/usr/bin/python3 - <<'PY'
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

def git(*args: str) -> str:
    return subprocess.check_output(('git', *args), text=True).strip()

head = git('rev-parse', 'HEAD')
status = subprocess.run(('git', 'status', '--porcelain'), text=True, capture_output=True, check=True).stdout
Path('.ci/dev/codex-next-gate.json').write_text(
    json.dumps(
        {
            'schema_version': 1,
            'completed_at': datetime.now(timezone.utc).isoformat(),
            'head_sha': head,
            'worktree_clean': status == '',
            'transcript_committed': False,
            'raw_logs_committed': False,
            'scope': 'option A absorbed-slope correction gate',
        },
        indent=2,
    ) + '\n',
    encoding='utf-8',
)
PY

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add .ci/dev/codex-next-gate.json
if ! git diff --cached --quiet; then
  git commit -m "ci(dev): record option A correction gate [skip ci]"
  for attempt in 1 2 3 4; do
    git fetch origin main
    git rebase origin/main
    if git push origin HEAD:main; then
      exit 0
    fi
    sleep $((2 * attempt))
  done
  exit 1
fi
