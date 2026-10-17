#!/bin/bash -l
#$ -P welfgr
#$ -j y
#$ -m n
# Resource requests are explicit at qsub: smoke 2, science/prepare 4 slots;
# measurement requires -pe omp28 28 -l num_proc=28,h_rt=01:15:00.
set -euo pipefail

module load miniconda/25.3.1
export SCC_MINICONDA_BIN="${SCC_MINICONDA_BIN:-$(dirname "$(command -v python)")}"
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
task_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec "${FEREG_BENCH_PYTHON:-$SCC_MINICONDA_BIN/python}" "$task_dir/compact_cells.py" "$@"
