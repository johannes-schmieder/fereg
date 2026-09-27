#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_sep_smoke
#$ -pe omp 4
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${FEREG_DEV_SNAPSHOT:?}"
export FEREG_DEV_PROFILE=ppml
# Use the project development profile, then the exact new boundary fixture.
source "$RUN_ROOT/source/benchmarks/scc/fereg_dev_smoke_job.sh"
cd "$RUN_ROOT/source"
python3 - "$RUN_ROOT/output/summary.json" "$FEREG_DEV_SNAPSHOT" <<'PYTHON'
import json,sys
from pathlib import Path
summary=json.loads(Path(sys.argv[1]).read_text())
assert summary["status"] == "success" and summary["tests_passed"] == 1
PYTHON
plugin_sha=$(sha256sum dist/fereg_linux.plugin | cut -d' ' -f1)
python3 scripts/run_stata_tests.py \
  --stata "$(command -v stata-mp)" \
  --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" \
  --plugin-sha256 "$plugin_sha" \
  --log-dir "$RUN_ROOT/output/separation-oracle" \
  --summary-json "$RUN_ROOT/output/separation-oracle.json" \
  tests/stata/development/feppml_separation_graph_oracle.do
printf 'FEREG_SEPARATION_SMOKE_SUCCESS job=%s snapshot=%s\n' "$JOB_ID" "$FEREG_DEV_SNAPSHOT"
