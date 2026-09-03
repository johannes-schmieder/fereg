#!/usr/bin/env bash
source /etc/bashrc
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 RUN_ROOT CANDIDATE_ROOT" >&2
    exit 2
fi
run_root=$1
candidate=$2
if [[ ! "$run_root" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
test -f "$run_root/receipts/submission.json"
test -f "$run_root/receipts/merge.json"
test ! -e "$run_root/RUN.SUCCESS"

module --force purge
module load python3/3.12.4
setup_job=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["jobs"]["setup"])' "$run_root/receipts/submission.json")
array_job=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["jobs"]["array"])' "$run_root/receipts/submission.json")
collector_job=$(python3 -I -B -S -c 'import json,sys; print(json.load(open(sys.argv[1]))["jobs"]["collector"])' "$run_root/receipts/submission.json")

setup_qacct=$(mktemp "$run_root/work/qacct-setup.XXXXXX")
array_qacct=$(mktemp "$run_root/work/qacct-array.XXXXXX")
collector_qacct=$(mktemp "$run_root/work/qacct-collector.XXXXXX")
trap 'rm -f "$setup_qacct" "$array_qacct" "$collector_qacct"' EXIT
qacct -j "$setup_job" > "$setup_qacct"
qacct -j "$array_job" > "$array_qacct"
qacct -j "$collector_job" > "$collector_qacct"

python3 -I -B -S "$candidate/scripts/ppml_solver_development.py" close \
    --run-root "$run_root" --setup-qacct "$setup_qacct" --array-qacct "$array_qacct" \
    --collector-qacct "$collector_qacct"
rm -f "$setup_qacct" "$array_qacct" "$collector_qacct"
trap - EXIT
printf 'FEPPML_SOLVER_DEV_CLOSE_SUCCESS run=%s\n' "$run_root"
