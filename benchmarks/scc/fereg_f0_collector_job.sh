#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_f0_collect
#$ -pe omp 1
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?RUN_ROOT is required}"
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "RUN_ROOT is not an exact fereg run directory" >&2
    exit 2
fi

module --force purge
module load python3/3.12.4

# -hold_jid proves completion only. The collector accepts the array only when
# every registered task receipt, sentinel, artifact identity, and cell hash is
# present; post-job closure separately requires complete qacct evidence.
python3 -I -B -S "$RUN_ROOT/input/harness/fereg_f0_scc.py" collect --run-root "$RUN_ROOT"
