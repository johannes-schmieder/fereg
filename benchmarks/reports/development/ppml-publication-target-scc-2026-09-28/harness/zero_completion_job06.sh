#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_target_zero
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${TMPDIR:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-publication-target-zero06 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 R/4.5.2 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
python3 - "$RUN_ROOT" <<'PRECHECK'
from pathlib import Path
import json,hashlib,sys
r=Path(sys.argv[1]);previous=r.with_name('20260928-publication-target-proof02b')
for name,digest in json.loads((r/'positive-bindings.json').read_text()).items():assert hashlib.sha256((previous/name).read_bytes()).hexdigest()==digest,name
c=json.loads((previous/'positive/results/comparison.json').read_text())
assert len(c['current'])==4 and all(v['pass_all'] and all(v['checks'].values()) for v in c['current'].values())
assert json.loads((previous/'positive/results/optimum-certificate.json').read_text())['coefficient_accuracy_certified']
print('EXISTING_POSITIVE_CHECKS_VALIDATED_NO_REFIT',flush=True)
PRECHECK
"$reference/env/python/bin/python" "$RUN_ROOT/zero/run.py" --run-root "$RUN_ROOT/zero" \
 --source /projectnb/welfgr/fereg/dev/20260928-publication-target-01/source \
 --baseline /projectnb/welfgr/fereg/dev/20260928-coefficient-stop-01/source \
 --reference "$reference" --scratch "$TMPDIR/proof-zero"
python3 - "$RUN_ROOT" <<'CHECK'
from pathlib import Path
import json,sys
r=Path(sys.argv[1]);d=json.loads((r/'zero/results/complete.json').read_text())
assert d['complete'] and d['oracle_pass'] and d['candidate_science'] and d['baseline_science']
(r/'complete.json').write_text(json.dumps(dict(complete=True,candidate_science=True,baseline_science=True,positive_job='7763492',positive_probe_process_failed=True,positive_failure_adjudicated='stale baseline-failure expectation; all planned positive numerical checks passed',zero_complete=True,no_timing_rank=True))+'\n')
CHECK
printf 'FEREG_PUBLICATION_TARGET_COMPLETION_SUCCESS job=%s\n' "$JOB_ID"
