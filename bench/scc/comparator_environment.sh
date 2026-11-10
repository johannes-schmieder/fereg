#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_comparators
#$ -pe omp 4
#$ -l mem_per_core=4G
#$ -l h_rt=02:00:00
#$ -j y
# Build the reused GLFixedEffectModels.jl/reghdfejl environment root from the
# pinned November 6 comparator installation, in a batch job (never a measured
# host). ENV_ROOT/input holds the source archive and its SHA256SUMS.
# Usage: qsub -v ENV_ROOT=...,ORIGIN=... -o ENV_ROOT/logs/build.sge comparator_environment.sh
set -euo pipefail
umask 022
: "${ENV_ROOT:?}" "${ORIGIN:?}"
cd "$ENV_ROOT"
sha256sum -c input/SHA256SUMS
mkdir source
tar -xf input/source.tar -C source
module purge
module load python3/3.12.4 julia/1.11.7 stata-mp/19
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONDONTWRITEBYTECODE=1
export JULIA_NUM_PRECOMPILE_TASKS=${NSLOTS:-4}
python3 source/bench/campaign_environment.py build "$ENV_ROOT" --origin "$ORIGIN"
(cd "$ENV_ROOT" && sha256sum -c --quiet SHA256SUMS)
printf 'CAMPAIGN_COMPARATOR_ENVIRONMENT_JOB_COMPLETE job=%s\n' "$JOB_ID"
