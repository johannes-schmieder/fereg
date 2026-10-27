#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_veneto_pair
#$ -j y
# Run the production task of several single-panel Veneto runs back to back on
# one whole node, so their absolute times share a host. Each run keeps its own
# preflight, budget, host observation and sentinel in its own logs.
# Usage: qsub -t 1-1 -v RUN_ROOTS="A B",UPSTREAM_JOBS="ja jb" panel_pair.sh
set -euo pipefail
: "${RUN_ROOTS:?}" "${UPSTREAM_JOBS:?}"
read -r -a roots <<< "$RUN_ROOTS"
read -r -a upstream <<< "$UPSTREAM_JOBS"
test "${#roots[@]}" = "${#upstream[@]}"
failed=0
for i in "${!roots[@]}"; do
    root=${roots[$i]}
    # A failed run stays visible in its own log; the next run still executes.
    RUN_ROOT=$root MODE=run UPSTREAM_JOB=${upstream[$i]} bash -l "$root/job.sh" \
        > "$root/logs/production.${SGE_TASK_ID}.sge" 2>&1 || failed=1
done
test "$failed" = 0
printf 'PANEL_PAIR_COMPLETE job=%s\n' "$JOB_ID"
