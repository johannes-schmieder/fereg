#!/usr/bin/env bash
set -eo pipefail
source /etc/bashrc
set -u
module --force purge
module load python3/3.12.4

if [ "$#" -ne 1 ]; then
    echo "usage: $0 /projectnb/welfgr/fereg/runs/RUN_ID" >&2
    exit 2
fi
RUN_ROOT=$1
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "RUN_ROOT is not an exact fereg run directory" >&2
    exit 2
fi

# Run only after the collector has left qstat. This bounded qacct reader waits
# for scheduler-accounting lag, validates every array task, and publishes the
# sole run-level SUCCESS marker last.
python3 "$RUN_ROOT/input/harness/fereg_f0_scc.py" close --run-root "$RUN_ROOT"
