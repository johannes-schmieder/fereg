#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_rebuild_bench
#$ -pe omp 28
#$ -l cpu_type=E5-2680v4
#$ -l mem_per_core=3G
#$ -l h_rt=02:00:00
#$ -j y
#$ -m n
set -eo pipefail
# SCC module initialization probes unset system variables.
set +u
source /etc/bashrc
module purge
module load python3/3.12.4 R/4.5.2 stata-mp/19
set -u
run_root=${1:?run root}
phase=${2:?setup or benchmark}
tier=${3:?tier}
[[ "$run_root" =~ ^/projectnb/welfgr/fereg/(dev|runs)/[A-Za-z0-9][A-Za-z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 28 ]]
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1
export OMP_NUM_THREADS=28
export PYTHONDONTWRITEBYTECODE=1
cd "$run_root"
if [[ "$phase" == setup ]]; then
    bash controller/setup.sh "$run_root"
    exit 0
fi
[[ "$phase" == benchmark && -f environment/SETUP.COMPLETE ]]
task=${SGE_TASK_ID:-1}
if [[ "$task" == undefined ]]; then task=1; fi
[[ "$task" =~ ^[1-9][0-9]*$ ]]
read -r cell threads < <(sed -n "${task}p" input/tasks.tsv)
[[ "$cell" =~ ^[a-z0-9-]+$ && "$threads" =~ ^(all|[0-9]+)$ ]]
args=(--tier "$tier" --engine legacy --environment "$run_root/environment"
      --root "$run_root/output/task-$task" --summary "$run_root/summaries/task-$task.csv"
      --stata "$(command -v stata-mp)" --stata-processors 4)
if [[ "$cell" != all ]]; then args+=(--cells "$cell"); fi
if [[ "$threads" != all ]]; then args+=(--threads-only "$threads"); fi
if [[ -f input/veneto-path.txt ]]; then args+=(--veneto-data "$(cat input/veneto-path.txt)"); fi
python3 source/bench/run.py "${args[@]}"
python3 - "$run_root/summaries/task-$task.csv" "$tier" <<'PY'
import csv,sys
rows=list(csv.DictReader(open(sys.argv[1])))
assert rows, 'empty benchmark summary'
if sys.argv[2]=='smoke':
    assert len(rows)==16 and all(r['status']=='ok' and r['accuracy']=='pass' and r['repetitions']=='5' for r in rows)
print('FEREG_BENCH_COMPLETE',len(rows),'groups',sum(r['status']!='ok' for r in rows),'command failures')
PY
