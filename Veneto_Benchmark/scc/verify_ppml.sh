#!/bin/bash -l
#$ -P welfgr
#$ -N veneto_ppml_fix
#$ -pe omp 14
#$ -l mem_per_core=3G
#$ -l h_rt=00:30:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${BENCHMARK_ROOT:?}" "${SOURCE_ID:?}"
module purge
module load stata-mp/19
module load python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=4 RAYON_NUM_THREADS=4
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$RUN_ROOT/build-target"
export CARGO_BUILD_JOBS=4 CARGO_NET_OFFLINE=true
cd "$RUN_ROOT"
sha256sum -c source.tar.sha256
mkdir source
 tar -xf source.tar -C source
python3 source/ci/run_fixture_profile.py --profile-config source/ci/stata_profiles.json --profile scc-dev-ppml --repo-root "$RUN_ROOT/source" --run-root "$RUN_ROOT/smoke" --summary-json "$RUN_ROOT/smoke/summary.json" --stata "$(command -v stata-mp)" --tested-sha "$SOURCE_ID" > smoke.log 2>&1
python3 "$RUN_ROOT/verify_ppml.py" "$RUN_ROOT" "$BENCHMARK_ROOT"
