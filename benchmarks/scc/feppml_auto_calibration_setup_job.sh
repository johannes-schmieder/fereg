#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_auto_setup
#$ -pe omp 4
#$ -l h_rt=00:45:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 4 ]]
test ! -e "$RUN_ROOT/work/SETUP.SUCCESS"
module --force purge
module load stata-mp/19 python3/3.12.4
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS="$NSLOTS" RAYON_NUM_THREADS="$NSLOTS"
export PATH="$CARGO_HOME/bin:$PATH"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
reference=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["reference_root"])' "$RUN_ROOT/input/config.json")
export CARGO_TARGET_DIR="$RUN_ROOT/work/target"
(cd "$candidate" && ./scripts/build_linux.sh)
for profile in local_science local_one_fe_science local_smoke; do
    python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" run-local \
        --profile "$profile" --candidate-root "$candidate" --reference-root "$reference" \
        --stata "$(command -v stata-mp)" --output-dir "$RUN_ROOT/work/$profile"
done
python3 -I -B -S "$candidate/scripts/ppml_auto_calibration.py" record-setup --run-root "$RUN_ROOT"
printf 'FEPPML_AUTO_SETUP_PASS job=%s\n' "$JOB_ID"
