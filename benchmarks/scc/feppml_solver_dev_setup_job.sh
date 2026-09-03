#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_solver_setup
#$ -pe omp 4
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -euo pipefail
source /etc/bashrc

: "${RUN_ROOT:?RUN_ROOT is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
if [[ "${NSLOTS:-}" != "4" || ! "${JOB_ID:-}" =~ ^[1-9][0-9]*$ ]]; then
    echo "invalid setup job identity" >&2
    exit 2
fi
test -f "$RUN_ROOT/input/config.json"
test ! -e "$RUN_ROOT/work/SETUP.SUCCESS"

module --force purge
module load stata-mp/19
module load python3/3.12.4

export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true
export PATH="$CARGO_HOME/bin:$PATH"
export OMP_NUM_THREADS=1
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
baseline=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1])).get("baseline_root") or "")' "$RUN_ROOT/input/config.json")
reference=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["reference_root"])' "$RUN_ROOT/input/config.json")
stata=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["stata"])' "$RUN_ROOT/input/config.json")

export CARGO_TARGET_DIR="$RUN_ROOT/work/target-candidate"
"$candidate/scripts/build_linux.sh"
if [[ -n "$baseline" ]]; then
    export CARGO_TARGET_DIR="$RUN_ROOT/work/target-baseline"
    "$baseline/scripts/build_linux.sh"
fi

export FEREG_PPML_SOLVER_MODE=prepare-reference
export FEREG_PPML_SOLVER_REFERENCE_ROOT="$reference"
export FEREG_PPML_SOLVER_STATA_LOG="$RUN_ROOT/logs/reference-setup.stata.log"
"$stata" -b do "$candidate/tests/stata/development/feppml_solver_development.do" \
    > "$RUN_ROOT/logs/reference-setup.log" 2>&1
grep -q '^FEREG_PPML_SOLVER_REFERENCE_PASS|' "$RUN_ROOT/logs/reference-setup.stata.log"

python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" record-setup \
    --run-root "$RUN_ROOT" --job-id "$JOB_ID"
printf 'FEPPML_SOLVER_DEV_SETUP_SUCCESS job=%s\n' "$JOB_ID"
