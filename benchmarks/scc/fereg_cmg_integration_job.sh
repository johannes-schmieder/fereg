#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cmg_dev
#$ -pe omp 28
#$ -l h_rt=00:30:00
#$ -l mem_per_core=3G
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SOURCE_SHA:?}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9._-]+$ ]]
[[ "$SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]
[[ "${NSLOTS:-}" == 28 ]]
cd "$RUN_ROOT"
test ! -e DONE
module load python3/3.12.4 stata-mp/19
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$RUN_ROOT/target"
export CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=1 RAYON_NUM_THREADS=4
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export FEREG_GIT_COMMIT="$SOURCE_SHA" FEREG_BUILD_DATE=2026-09-17
mkdir -p logs
qstat -j "$JOB_ID" > logs/allocation.txt
sha256sum -c source.sha256
cargo --version > logs/toolchain.txt
rustc --version >> logs/toolchain.txt
cd source
# Fetch only the exact locked closure, then keep all compilation offline.
cargo fetch --locked > ../logs/fetch.txt 2>&1
cargo test --locked --offline -p fereg-core --lib fe::cmg -- --test-threads=1 > ../logs/cmg.txt 2>&1
cargo test --locked --offline -p fereg-core --lib ppml::categorical_fwl_cmg -- --test-threads=1 > ../logs/ppml-cmg.txt 2>&1
cargo test --locked --offline -p fereg-core --lib ppml::prepared_irls::tests -- --test-threads=1 > ../logs/irls.txt 2>&1
cargo test --locked --offline -p fereg-plugin --lib -- --test-threads=1 > ../logs/plugin.txt 2>&1
bash scripts/build_linux.sh > ../logs/build.txt 2>&1
bash tests/mock_spi/run_actual_unix_plugin_test.sh dist/fereg_linux.plugin > ../logs/loader.txt 2>&1
python3 ci/run_fixture_profile.py --profile-config ci/stata_profiles.json --profile scc-dev-shared \
  --repo-root "$RUN_ROOT/source" --run-root "$RUN_ROOT/licensed" --summary-json "$RUN_ROOT/licensed/summary.json" \
  --stata "$(command -v stata-mp)" --tested-sha "$SOURCE_SHA" > ../logs/licensed.txt 2>&1
python3 - "$RUN_ROOT/licensed/summary.json" <<'PY'
import json,sys
s=json.load(open(sys.argv[1]))
assert s['status']=='success' and s['tests_failed']==0 and s['tests_passed']==2,s
PY
sha256sum dist/fereg_linux.plugin > ../logs/artifact.sha256
cd "$RUN_ROOT"
sha256sum -c source.sha256 > logs/final-source.txt
printf 'FEREG_CMG_INTEGRATION_PASS source=%s job=%s\n' "$SOURCE_SHA" "$JOB_ID" > DONE
cat DONE
