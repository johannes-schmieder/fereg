#!/usr/bin/env bash
source /etc/bashrc
set -euo pipefail

if [[ $# -lt 5 || $# -gt 6 ]]; then
    echo "usage: $0 RUN_ROOT CANDIDATE_ROOT REFERENCE_ROOT LOCAL_SCIENCE LOCAL_SMOKE [BASELINE_ROOT]" >&2
    exit 2
fi
run_root=$1
candidate=$2
reference=$3
local_science=$4
local_smoke=$5
baseline=${6:-}
if [[ ! "$run_root" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi

module --force purge
module load stata-mp/19
module load python3/3.12.4

prepare=(python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" prepare-run
    --run-root "$run_root" --candidate-root "$candidate" --reference-root "$reference"
    --local-science "$local_science" --local-smoke "$local_smoke"
    --stata "$(command -v stata-mp)" --repetitions 3)
if [[ -n "$baseline" ]]; then
    prepare+=(--baseline-root "$baseline")
fi
"${prepare[@]}"

setup_raw=$(qsub -terse -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$candidate/benchmarks/scc/feppml_solver_dev_setup_job.sh")
setup_job=${setup_raw%%.*}
[[ "$setup_job" =~ ^[1-9][0-9]*$ ]]
array_raw=$(qsub -terse -hold_jid "$setup_job" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$candidate/benchmarks/scc/feppml_solver_dev_job.sh")
array_job=${array_raw%%.*}
[[ "$array_job" =~ ^[1-9][0-9]*$ ]]
collector_raw=$(qsub -terse -hold_jid "$array_job" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$candidate/benchmarks/scc/feppml_solver_dev_collector_job.sh")
collector_job=${collector_raw%%.*}
[[ "$collector_job" =~ ^[1-9][0-9]*$ ]]

python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" record-submission \
    --run-root "$run_root" --setup-job "$setup_job" --array-job "$array_job" \
    --collector-job "$collector_job"
printf 'setup=%s array=%s collector=%s run=%s\n' "$setup_job" "$array_job" "$collector_job" "$run_root"
