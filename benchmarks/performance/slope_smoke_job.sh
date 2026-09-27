#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ols_slope_smoke
#$ -pe omp 4
#$ -l mem_per_core=4G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${FEREG_DEV_SNAPSHOT:?}" "${BASELINE_SOURCE:?}" "${REFERENCE_RUN:?}"
export FEREG_DEV_PROFILE=ols
source "$RUN_ROOT/source/benchmarks/scc/fereg_dev_smoke_job.sh"
cd "$RUN_ROOT/source"
plugin_sha=$(sha256sum dist/fereg_linux.plugin | cut -d' ' -f1)
python3 scripts/run_stata_tests.py \
    --stata "$(command -v stata-mp)" \
    --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 "$plugin_sha" \
    --log-dir "$RUN_ROOT/output/slope-oracle" --summary-json "$RUN_ROOT/output/slope-oracle.json" \
    tests/stata/development/ols_multipartition_schur_oracle.do
module load R/4.5.2
export PYTHONDONTWRITEBYTECODE=1
"$REFERENCE_RUN/env/python/bin/python" benchmarks/performance/slope_rotation.py \
    --run "$RUN_ROOT" --plan "$RUN_ROOT/registration.json" \
    --baseline "$BASELINE_SOURCE" --candidate "$RUN_ROOT/source" --reference "$REFERENCE_RUN" \
    --output "$RUN_ROOT/smoke-probe" --smoke
printf 'FEREG_OLS_SLOPE_SMOKE_SUCCESS job=%s snapshot=%s\n' "$JOB_ID" "$FEREG_DEV_SNAPSHOT"
