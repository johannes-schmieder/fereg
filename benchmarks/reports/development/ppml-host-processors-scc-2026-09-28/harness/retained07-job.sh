#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_host_retained
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:15:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-host-retained-07 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=4 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
base=/projectnb/welfgr/fereg/dev/20260928-host-processors-02/source
mkdir "$RUN_ROOT/source"
rsync -rlt --exclude target --exclude .git "$base/" "$RUN_ROOT/source/"
cp "$RUN_ROOT/_fereg_estimate_ppml.ado" "$RUN_ROOT/source/stata/"
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import hashlib,json,sys
r=Path(sys.argv[1]);m=json.loads((r/'retained-source.json').read_text())
for f,h in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==h,f
for f,h in m['files'].items():assert hashlib.sha256((r/'source'/f).read_bytes()).hexdigest()==h,f
assert hashlib.sha256((r/'source/dist/fereg_linux.plugin').read_bytes()).hexdigest()=='d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116'
GATE
mkdir "$RUN_ROOT/output"
cd "$RUN_ROOT/source"
python3 scripts/run_stata_tests.py --stata "$(command -v stata-mp)" \
 --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116 \
 --log-dir "$RUN_ROOT/output/fixtures" --summary-json "$RUN_ROOT/output/summary.json" \
 ci/feppml_smoke.do tests/stata/development/feppml_host_processors.do
python3 scripts/run_stata_break_test.py --stata "$(command -v stata-mp)" \
 --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116 \
 --fixture-kind ppml --timeout 90 > "$RUN_ROOT/output/break.log" 2>&1
python3 - "$RUN_ROOT" <<'GATE'
from pathlib import Path
import json,sys
r=Path(sys.argv[1]);d=json.loads((r/'output/summary.json').read_text())
assert d['status']=='success' and d['tests_passed']==2 and d['tests_failed']==0
assert 'PASS: END_FIXTURE|estimate_ppml_eclass_break|rc=0' in (r/'output/break.log').read_text()
GATE
printf 'FEREG_HOST_RETAINED_SUCCESS job=%s\n' "$JOB_ID"
