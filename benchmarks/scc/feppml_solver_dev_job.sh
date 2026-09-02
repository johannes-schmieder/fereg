#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_solver_dev
#$ -pe omp 8
#$ -l h_rt=04:00:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u

: "${RUN_ROOT:?RUN_ROOT is required}"
: "${FEREG_DEV_SNAPSHOT:?FEREG_DEV_SNAPSHOT is required}"
: "${FEREG_PPML_CANDIDATE:?FEREG_PPML_CANDIDATE must be cg, lsmr, or cmg}"

if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
if [[ ! "$FEREG_DEV_SNAPSHOT" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
    echo "FEREG_DEV_SNAPSHOT must be a 40- or 64-character source identity" >&2
    exit 2
fi
case "$FEREG_PPML_CANDIDATE" in
    cg|lsmr|cmg) ;;
    *) echo "FEREG_PPML_CANDIDATE must be cg, lsmr, or cmg" >&2; exit 2 ;;
esac
if [[ "${NSLOTS:-}" != "8" ]]; then
    echo "PPML solver development confirmation requires exactly eight slots" >&2
    exit 2
fi

SOURCE_ROOT="$RUN_ROOT/source"
REFERENCE_ROOT="$RUN_ROOT/references"
OUTPUT_ROOT="$RUN_ROOT/output"
SUCCESS="$OUTPUT_ROOT/FEPPML_SOLVER_DEV.SUCCESS"
test -f "$SOURCE_ROOT/scripts/ppml_solver_development.py"
test -f "$SOURCE_ROOT/benchmarks/manifests/feppml_solver_development_v1.json"
test -d "$REFERENCE_ROOT/ppmlhdfe/src"
test ! -e "$SUCCESS"
mkdir -p "$OUTPUT_ROOT" "$RUN_ROOT/work/target"

module --force purge
module load stata-mp/19
module load python3/3.12.4

export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="$RUN_ROOT/work/target"
export PATH="$CARGO_HOME/bin:$PATH"
export OMP_NUM_THREADS=8
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

"$SOURCE_ROOT/scripts/build_linux.sh"

RUN_OUTPUT=$(python3 "$SOURCE_ROOT/scripts/ppml_solver_development.py" run-local \
    --profile scale_thread \
    --stata "$(command -v stata-mp)" \
    --reference-root "$REFERENCE_ROOT" \
    --output-dir "$OUTPUT_ROOT/runs" \
    --repetitions 5 \
    --n-override 1000000 \
    --limit 12 \
    --arms map "$FEREG_PPML_CANDIDATE" ppmlhdfe)

python3 "$SOURCE_ROOT/scripts/ppml_solver_development.py" analyze \
    "$RUN_OUTPUT/run.json" \
    --output "$OUTPUT_ROOT/analysis.json"

printf 'snapshot=%s\njob_id=%s\ncandidate=%s\nrun=%s\n' \
    "$FEREG_DEV_SNAPSHOT" "${JOB_ID:-unknown}" "$FEREG_PPML_CANDIDATE" "$RUN_OUTPUT" \
    > "$SUCCESS"
printf 'FEPPML_SOLVER_DEV_SUCCESS candidate=%s snapshot=%s\n' \
    "$FEREG_PPML_CANDIDATE" "$FEREG_DEV_SNAPSHOT"
