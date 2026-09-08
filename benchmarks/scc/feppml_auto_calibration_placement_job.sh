#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_placement
#$ -pe omp 28
#$ -l h_rt=00:15:00
#$ -l mem_per_core=4G
#$ -l m_core=28
#$ -l num_proc=28
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 28 ]]
module --force purge
module load stata-mp/19 python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
reference=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["reference_root"])' "$RUN_ROOT/input/config.json")
controller=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["controller"]["root"])' "$RUN_ROOT/input/config.json")
test ! -e "$RUN_ROOT/work/qhost-placement.txt"
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type,num_proc > "$RUN_ROOT/work/qhost-placement.txt"
python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" run-local \
    --profile local_one_fe_science --candidate-root "$candidate" --reference-root "$reference" \
    --stata "$(command -v stata-mp)" --output-dir "$RUN_ROOT/work/placement-science"
python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" placement-check --run-root "$RUN_ROOT"
printf 'FEPPML_AUTO_PLACEMENT_PASS job=%s\n' "$JOB_ID"
