#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_auto_cal
#$ -pe omp 28
#$ -l h_rt=12:00:00
#$ -l mem_per_core=4G
#$ -l m_core=28
#$ -l num_proc=28
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 28 && "${SGE_TASK_ID:-}" =~ ^[1-9][0-9]*$ ]]
[[ "${STAGE:-}" == scc_train || "${STAGE:-}" == scc_holdout ]]
test -f "$RUN_ROOT/work/SETUP.SUCCESS"
if [[ "$STAGE" == scc_holdout ]]; then test -f "$RUN_ROOT/work/POLICY.SUCCESS"; fi
module --force purge
module load stata-mp/19 python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export FEREG_SOLVER_TRACE=1
candidate=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["candidate_root"])' "$RUN_ROOT/input/config.json")
reference=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["reference_root"])' "$RUN_ROOT/input/config.json")
controller=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["controller"]["root"])' "$RUN_ROOT/input/config.json")
logical_task=$(python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" scheduled-task --run-root "$RUN_ROOT" --stage "$STAGE" --index "$SGE_TASK_ID")
task=$(printf '%03d' "$logical_task")
test ! -e "$RUN_ROOT/work/qhost-$STAGE-$task.txt"
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type > "$RUN_ROOT/work/qhost-$STAGE-$task.txt"
policy_args=()
if [[ "$STAGE" == scc_holdout ]]; then policy_args=(--policy "$RUN_ROOT/input/policy.json"); fi
python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" run --stage "$STAGE" --run-root "$RUN_ROOT" \
    --task "$logical_task" --candidate-root "$candidate" --reference-root "$reference" \
    --stata "$(command -v stata-mp)" "${policy_args[@]}" \
    --output-dir "$RUN_ROOT/shards/$STAGE/task-$task"
printf 'FEPPML_AUTO_TASK_PASS stage=%s task=%s scheduler_task=%s job=%s\n' "$STAGE" "$logical_task" "$SGE_TASK_ID" "$JOB_ID"
