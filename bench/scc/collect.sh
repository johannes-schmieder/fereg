#!/bin/bash
set -euo pipefail
remote=${1:?remote run root}
local_root=${2:?new local collection directory}
summary=${3:?summary output path}
[[ "$remote" =~ ^/projectnb/welfgr/fereg/(dev|runs)/[A-Za-z0-9][A-Za-z0-9._-]{0,95}$ ]]
[[ ! -e "$local_root" && ! -e "$summary" ]]
mkdir -p "$local_root/summaries"
rsync -a "scc:$remote/jobs.tsv" "$local_root/"
rsync -a "scc:$remote/input/tasks.tsv" "scc:$remote/input/source.json" "$local_root/"
rsync -a "scc:$remote/source/bench/suite.toml" "$local_root/"
while IFS=$'\t' read -r phase job tasks; do
    [[ "$phase" =~ ^(setup|benchmark)$ && "$job" =~ ^[0-9]+$ && "$tasks" =~ ^[1-9][0-9]*$ ]]
    ssh scc "qacct -j '$job'" > "$local_root/$phase-qacct.txt"
done < "$local_root/jobs.tsv"
ssh scc "test -f '$remote/environment/SETUP.COMPLETE'; grep -h 'FEREG_BENCH_COMPLETE' '$remote'/logs/*" > "$local_root/completion.txt"
rsync -a "scc:$remote/summaries/" "$local_root/summaries/"
"${FEREG_PYTHON_BIN:-python3}" "$(dirname "${BASH_SOURCE[0]}")/collect.py" "$local_root" --output "$summary"
