#!/bin/bash -l
#$ -P welfgr
#$ -N veneto_ppml_cert
#$ -pe omp 14
#$ -l mem_per_core=3G
#$ -l h_rt=00:30:00
#$ -j y
set -euo pipefail
umask 077
: "${BENCHMARK_ROOT:?}" "${DIAGNOSTIC_ROOT:?}"
module purge
module load stata-mp/19
module load python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=14 RAYON_NUM_THREADS=14
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$DIAGNOSTIC_ROOT/build-target"
export CARGO_BUILD_JOBS=4 CARGO_NET_OFFLINE=true
export FEREG_GIT_COMMIT=d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd
export FEREG_BUILD_DATE=2026-09-12
cd "$DIAGNOSTIC_ROOT"
sha256sum -c instrumented-source.tar.sha256
mkdir candidate
tar -xf instrumented-source.tar -C candidate
bash candidate/scripts/build_linux.sh > build.log 2>&1
sha256sum candidate/dist/fereg_linux.plugin > instrumented-plugin.sha256
python3 "$DIAGNOSTIC_ROOT/trace_certificates.py" "$BENCHMARK_ROOT" "$DIAGNOSTIC_ROOT" "$DIAGNOSTIC_ROOT/candidate"
