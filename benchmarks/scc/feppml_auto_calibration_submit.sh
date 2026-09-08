#!/usr/bin/env bash
source /etc/bashrc
set -euo pipefail
module --force purge
module load stata-mp/19 python3/3.12.4
if [[ $# == 2 && "$1" == --recover ]]; then
    run_root=$2
    [[ "$run_root" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
    controller=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["controller"]["root"])' "$run_root/input/config.json")
    python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" accept-placement --run-root "$run_root"
    setup=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["job_id"])' "$run_root/receipts/setup.json")
    original_train=$(python3 -I -B -S -c 'import json,sys; c=json.load(open(sys.argv[1])); print(json.load(open(c["recovery"]["original_root"]+"/receipts/submission.json"))["jobs"]["scc_train"])' "$run_root/input/config.json")
    train_tasks=$(python3 -I -B -S -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["recovery"]["retry_tasks"]))' "$run_root/input/config.json")
else
[[ $# == 4 ]] || { echo "usage: $0 RUN_ROOT SOURCE REFERENCE LOCAL_VALIDATION | --recover PREPARED_RUN_ROOT" >&2; exit 2; }
run_root=$1
candidate=$2
reference=$3
local_validation=$4
controller=$candidate
[[ "$run_root" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
python3 -I -B -S "$candidate/scripts/ppml_auto_calibration.py" prepare-scc \
    --run-root "$run_root" --candidate-root "$candidate" --reference-root "$reference" \
    --local-validation "$local_validation" --stata "$(command -v stata-mp)"
train_tasks=$(python3 -I -B -S -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["tasks"]["scc_train"]))' "$run_root/input/config.json")
setup_raw=$(qsub -terse -h -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$candidate/benchmarks/scc/feppml_auto_calibration_setup_job.sh")
setup=${setup_raw%%.*}
[[ "$setup" =~ ^[1-9][0-9]*$ ]]
printf 'accepted setup=%s run=%s\n' "$setup" "$run_root"
fi
test ! -e "$run_root/receipts/submission.json"
holdout_tasks=$(python3 -I -B -S -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["tasks"]["scc_holdout"]))' "$run_root/input/config.json")
train_raw=$(qsub -terse -h -hold_jid "$setup" -t "1-$train_tasks" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root",STAGE=scc_train "$controller/benchmarks/scc/feppml_auto_calibration_job.sh")
train=${train_raw%%.*}
[[ "$train" =~ ^[1-9][0-9]*$ ]]
printf 'accepted train=%s run=%s\n' "$train" "$run_root"
policy_dependencies=$train
if [[ -n "${original_train:-}" ]]; then policy_dependencies="$train,$original_train"; fi
policy_raw=$(qsub -terse -h -hold_jid "$policy_dependencies" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$controller/benchmarks/scc/feppml_auto_calibration_policy_job.sh")
policy=${policy_raw%%.*}
[[ "$policy" =~ ^[1-9][0-9]*$ ]]
printf 'accepted policy=%s run=%s\n' "$policy" "$run_root"
holdout_raw=$(qsub -terse -h -hold_jid "$policy" -t "1-$holdout_tasks" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root",STAGE=scc_holdout "$controller/benchmarks/scc/feppml_auto_calibration_job.sh")
holdout=${holdout_raw%%.*}
[[ "$holdout" =~ ^[1-9][0-9]*$ ]]
printf 'accepted holdout=%s run=%s\n' "$holdout" "$run_root"
collector_raw=$(qsub -terse -h -hold_jid "$holdout" -wd "$run_root" -o "$run_root/logs" -v RUN_ROOT="$run_root" "$controller/benchmarks/scc/feppml_auto_calibration_collect_job.sh")
collector=${collector_raw%%.*}
[[ "$collector" =~ ^[1-9][0-9]*$ ]]
printf 'accepted collector=%s run=%s\n' "$collector" "$run_root"
python3 -I -B -S "$controller/scripts/ppml_auto_calibration.py" record-submission \
    --run-root "$run_root" --setup-job "$setup" --scc-train-job "$train" --policy-job "$policy" \
    --scc-holdout-job "$holdout" --collector-job "$collector"
qrls "$train" "$policy" "$holdout" "$collector"
if [[ -z "${original_train:-}" ]]; then qrls "$setup"; fi
printf 'setup=%s train=%s policy=%s holdout=%s collector=%s run=%s\n' "$setup" "$train" "$policy" "$holdout" "$collector" "$run_root"
