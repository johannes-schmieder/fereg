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
history, AGENTS.md, STATUS.md, exact-SHA receipts, and current workflow state
before editing. The project is development-only. Public stfeols, stfepois,
estimate-ols, and estimate-ppml must remain fail-closed unless every governing
scientific, transaction, parity, performance, and activation gate is actually
complete and exact-green.

Your task in this invocation is ONE coherent next gate toward completing
stfeols, selected from the first incomplete machine-verifiable gate in the live
STATUS.md. Do not redo an accepted gate merely because this prompt names an
older checkpoint. At the expected current state, the twelve sealed numeric
categorical FE-interaction cases and the unchanged frozen 30-executable/10-
blocked hidden OLS projection have already been accepted exact-green. Verify
that evidence first. If STATUS.md has advanced further, follow its newer active
checkpoint instead.

At the expected current checkpoint, reconcile and activate final-sample
categorical model rebasing on main. Use the preserved staged implementation and
validation contract as evidence, not as an excuse to bypass review. Inspect at
least prepared_categorical_rebase.rs, prepared_categorical_data.rs,
activate_final_sample_rebase.py, the staging/validation contract tests, and the
stale validation workflow. Remove obsolete deleted-branch assumptions. Ensure
that factor levels removed by authoritative singleton/sample compaction are
handled on the final sample, that the compact public-to-original coefficient
report plan is validated and transported correctly through every affected
hidden iid/robust/clustered path, and that continuous-source storage identity,
omission/base-level semantics, canonical ordering, allocation failure, and
failure atomicity remain intact. Add focused independent tests for the actual
activation. Do not broaden the frozen executable partition or enable public
dispatch as part of this gate.

Use maximum care and reasoning effort. Treat reghdfe comparison as necessary
but not sufficient; preserve independent mathematical/oracle checks. Run all
relevant Rust, Python, C/loader, and licensed Stata tests locally on this runner.
Only call a source accepted when an exact-SHA receipt records overall, Rust, and
licensed Stata success. Make small coherent commits directly to main and push
after meaningful changes. Do not commit raw licensed logs, local paths, secrets,
or restricted data. Update STATUS.md only after exact-green evidence exists,
and bind it to the reachable accepted source and receipt.

Stop this invocation after one coherent gate is exact-green and pushed, or at
the first genuinely scientific/API/architectural ambiguity requiring the user.
For an ordinary implementation or debugging decision, use best judgment and
continue. Leave a concise final report naming source SHA, receipt path/commit,
tests, files changed, and the exact next gate.
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
            'scope': 'one coherent next stfeols gate',
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
  git commit -m "ci(dev): record bounded coding-agent gate [skip ci]"
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
