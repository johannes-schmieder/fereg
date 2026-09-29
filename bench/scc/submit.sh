#!/bin/bash
# Local deployment; only exact new namespaces are created, with no deletion.
set -euo pipefail
run_id=${1:?run id}
tier=${2:?smoke, standard or full}
staging=${3:?local input staging directory}
[[ "$run_id" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$ ]]
[[ "$tier" =~ ^(smoke|standard|full)$ ]]
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
lane=runs
if [[ "$tier" == smoke ]]; then lane=dev; fi
remote="/projectnb/welfgr/fereg/$lane/$run_id"
[[ -f "$staging/SHA256SUMS" && -f "$staging/tasks.tsv" ]]
ssh scc bash -s -- "$remote" <<'PREFLIGHT'
set -euo pipefail
run_root=$1
available=$(df -Pk /projectnb/welfgr/fereg | awk 'NR==2 {print $4}')
[[ "$available" =~ ^[0-9]+$ && "$available" -gt 10485760 ]] || { echo 'less than 10 GiB available for staging/build' >&2; exit 1; }
test ! -e "$run_root"
mkdir -p "$run_root/input" "$run_root/controller" "$run_root/logs" "$run_root/output" "$run_root/summaries"
PREFLIGHT
rsync -a "$staging/" "scc:$remote/input/"
rsync -a "$root/bench/scc/" "scc:$remote/controller/"
ssh scc bash -s -- "$remote" "$tier" <<'REMOTE'
set -euo pipefail
run_root=$1
tier=$2
cd "$run_root"
sha256sum -c input/SHA256SUMS
[[ ! -e jobs.tsv ]]
setup=$(qsub -terse -P welfgr -wd "$run_root" -o "$run_root/logs/setup.log" -l h_rt=02:00:00 controller/job.sh "$run_root" setup "$tier")
setup=${setup%%.*}
[[ "$setup" =~ ^[0-9]+$ ]]
printf 'setup\t%s\t1\n' "$setup" > jobs.tsv
tasks=$(wc -l < input/tasks.tsv)
wall=02:00:00
if [[ "$tier" == full ]]; then wall=22:00:00; fi
benchmark=$(qsub -terse -P welfgr -wd "$run_root" -o "$run_root/logs" -hold_jid "$setup" -t "1-$tasks" -l "h_rt=$wall" controller/job.sh "$run_root" benchmark "$tier")
benchmark=${benchmark%%.*}
[[ "$benchmark" =~ ^[0-9]+$ ]]
printf 'benchmark\t%s\t%s\n' "$benchmark" "$tasks" >> jobs.tsv
cat jobs.tsv
REMOTE
