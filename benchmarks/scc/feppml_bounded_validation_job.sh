#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_bounded
#$ -pe omp28 28
#$ -l h_rt=00:30:00
#$ -l mem_per_core=3G
#$ -l m_core=28
#$ -l num_proc=28
#$ -j y
#$ -m a

source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
: "${FEREG_BOUNDED_STAGE:?stage is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "$FEREG_BOUNDED_STAGE" == smoke || "$FEREG_BOUNDED_STAGE" == main ]]
[[ "${NSLOTS:-}" == 28 && "${JOB_ID:-}" =~ ^[1-9][0-9]*$ ]]
[[ "${SGE_TASK_ID:-}" =~ ^[1-9][0-9]*$ ]]
module --force purge
module load stata-mp/19 python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export RAYON_NUM_THREADS="$NSLOTS"
task_name=$(printf 'task-%03d' "$SGE_TASK_ID")
qhost_path="$RUN_ROOT/logs/qhost-$FEREG_BOUNDED_STAGE-$task_name.txt"
test ! -e "$qhost_path"
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type > "$qhost_path"
python3 -I -B -S "$RUN_ROOT/controller/scripts/ppml_bounded_validation.py" run \
    --config "$RUN_ROOT/input/config.json" \
    --candidate /projectnb/welfgr/fereg/dev/20260907T002400Z-within93699e6/source \
    --reference /projectnb/welfgr/fereg/dev/20260907T002400Z-within93699e6/reference \
    --stata "$(command -v stata-mp)" --task "$SGE_TASK_ID" --qhost "$qhost_path" \
    --output "$RUN_ROOT/output/$FEREG_BOUNDED_STAGE/$task_name"
printf 'BOUNDED_JOB_CAPTURE_COMPLETE job=%s task=%s\n' "$JOB_ID" "$SGE_TASK_ID"
