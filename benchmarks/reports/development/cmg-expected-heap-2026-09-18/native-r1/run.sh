#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cmg_heap
#$ -pe omp 28
#$ -l num_proc=28
#$ -l h_rt=00:20:00
#$ -l mem_per_core=3G
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}"
[[ "$RUN_ROOT" == ${DEV_ROOT}/cmg-heap-20260918-r1 ]]
[[ "${NSLOTS:-}" == 28 ]]
cd "$RUN_ROOT"
test ! -e DONE
export CARGO_HOME=${TOOLCHAIN_ROOT}/cargo RUSTUP_HOME=${TOOLCHAIN_ROOT}/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=1 RAYON_NUM_THREADS=4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
export FEREG_BUILD_DATE=2026-09-18 CARGO_NET_OFFLINE=true
module load python3/3.12.4
for variant in candidate baseline; do
 cd "$RUN_ROOT/$variant"
 sha256sum -c source.sha256 > ../logs/$variant-source-before.txt
 export CARGO_TARGET_DIR="$RUN_ROOT/target-$variant"
 export FEREG_GIT_COMMIT="e7ad6501-cmg-heap-r1-$variant"
 cd source
 if [[ "$variant" == candidate ]]; then
  cargo test --locked --offline -p fereg-core --lib memory::workspace_peak -- --test-threads=1 > ../../logs/workspace.txt 2>&1
  cargo test --locked --offline -p fereg-core --lib fe::cmg -- --test-threads=1 > ../../logs/cmg.txt 2>&1
  cargo test --locked --offline -p fereg-core --lib ppml::categorical_fwl_cmg -- --test-threads=1 > ../../logs/ppml-cmg.txt 2>&1
  cargo test --locked --offline -p fereg-core --lib ppml::prepared_irls::tests::cmg -- --test-threads=1 > ../../logs/ppml-frames.txt 2>&1
  FEREG_CMG_ORDINARY=1 cargo test --locked --offline -p fereg-plugin --lib -- --test-threads=1 > ../../logs/plugin.txt 2>&1
 fi
 bash scripts/build_linux.sh > ../../logs/$variant-build.txt 2>&1
 sha256sum dist/fereg_linux.plugin > ../../logs/$variant-artifact.sha256
 cd ..
 sha256sum -c source.sha256 > ../logs/$variant-source-after.txt
 done
cd "$RUN_ROOT"
printf 'FEREG_CMG_HEAP_NATIVE_PASS job=%s\n' "$JOB_ID" > DONE
cat DONE
