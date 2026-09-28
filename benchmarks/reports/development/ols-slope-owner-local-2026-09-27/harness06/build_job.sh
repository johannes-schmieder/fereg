#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_slope_box_build
#$ -pe omp 4
#$ -l mem_per_core=4G,h_rt=00:20:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${FEREG_DEV_SNAPSHOT:?}"
export FEREG_DEV_PROFILE=ols
source "$RUN_ROOT/source/benchmarks/scc/fereg_dev_smoke_job.sh"
cd "$RUN_ROOT/source"
plugin_sha=$(sha256sum dist/fereg_linux.plugin | cut -d' ' -f1)
python3 scripts/run_stata_tests.py \
 --stata "$(command -v stata-mp)" --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 "$plugin_sha" \
 --log-dir "$RUN_ROOT/output/slope-oracle" --summary-json "$RUN_ROOT/output/slope-oracle.json" \
 tests/stata/development/ols_multipartition_schur_oracle.do
printf 'FEREG_SLOPE_BOX_BUILD_SUCCESS job=%s\n' "$JOB_ID"
