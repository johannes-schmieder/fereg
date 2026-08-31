#!/usr/bin/env bash
#$ -cwd
#$ -j y
set -eo pipefail

: "${RUN_ROOT:?RUN_ROOT is required}"
: "${FEREG_SOURCE_ROOT:?FEREG_SOURCE_ROOT is required}"
: "${COMPARATOR_ROOT:?COMPARATOR_ROOT is required}"
: "${BENCH_MODE:?BENCH_MODE is required}"

source /etc/bashrc
set -u
module --force purge
module load stata-mp/19
module load python3/3.12.4
python3 "$RUN_ROOT/source/benchmarks/scc/paper_akm_runner.py" \
  --run-root "$RUN_ROOT" \
  --source-root "$FEREG_SOURCE_ROOT" \
  --comparator-root "$COMPARATOR_ROOT" \
  --mode "$BENCH_MODE"
