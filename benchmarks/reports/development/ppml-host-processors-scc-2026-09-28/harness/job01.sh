#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_host_validate
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:15:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${FEREG_DEV_SNAPSHOT:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-host-processors-01 && "$NSLOTS" == 4 ]]
base=/projectnb/welfgr/fereg/dev/20260928-publication-target-01/source
mkdir "$RUN_ROOT/source"
rsync -rlt --exclude target --exclude dist --exclude .git "$base/" "$RUN_ROOT/source/"
mkdir "$RUN_ROOT/source/dist"
cp "$base/dist/fereg_linux.plugin" "$RUN_ROOT/source/dist/"
python3 - "$RUN_ROOT" <<'CHECK'
from pathlib import Path
import json,hashlib,shutil,sys
r=Path(sys.argv[1]);m=json.loads((r/'staged/candidate-source.json').read_text())
for f,h in json.loads((r/'staged/deployment.json').read_text()).items():assert hashlib.sha256((r/'staged'/f).read_bytes()).hexdigest()==h,f
for f in m['overlay_files']:
 p=r/'source'/f;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(r/'staged/overlay'/f,p)
for f,h in m['files'].items():assert hashlib.sha256((r/'source'/f).read_bytes()).hexdigest()==h,f
assert hashlib.sha256((r/'source/dist/fereg_linux.plugin').read_bytes()).hexdigest()=='94925723e066bdbacf067a66524ff08918d6f3612fcbf557a1c4f1be3091a00e'
print('Verified package snapshot',m['snapshot'],'with unchanged accepted native plugin')
CHECK
module purge
module load stata-mp/19 python3/3.12.4
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=4 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
mkdir "$RUN_ROOT/output"
cd "$RUN_ROOT/source"
python3 -m unittest discover -s tests/python -p test_ppml_stata_transaction_contract.py > "$RUN_ROOT/output/static.log" 2>&1
python3 scripts/run_stata_tests.py --stata "$(command -v stata-mp)" \
 --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 94925723e066bdbacf067a66524ff08918d6f3612fcbf557a1c4f1be3091a00e \
 --log-dir "$RUN_ROOT/output/fixtures" --summary-json "$RUN_ROOT/output/summary.json" \
 ci/feppml_smoke.do \
 tests/stata/development/feppml_host_processors.do \
 tests/stata/parity_ppmlhdfe/feppml_one_fe_passes_oracle.do \
 tests/stata/development/feppml_three_partition_slope_oracle.do \
 tests/stata/development/ols_multipartition_schur_oracle.do
python3 scripts/run_stata_break_test.py --stata "$(command -v stata-mp)" \
 --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 94925723e066bdbacf067a66524ff08918d6f3612fcbf557a1c4f1be3091a00e \
 --fixture-kind ppml --timeout 90 > "$RUN_ROOT/output/break.log" 2>&1
python3 - "$RUN_ROOT" <<'CHECK'
from pathlib import Path
import json,sys
r=Path(sys.argv[1]);d=json.loads((r/'output/summary.json').read_text())
assert d['status']=='success' and d['tests_passed']==5 and d['tests_failed']==0
assert 'PASS: END_FIXTURE|estimate_ppml_eclass_break|rc=0' in (r/'output/break.log').read_text()
assert 'Ran 17 tests' in (r/'output/static.log').read_text() and '\nOK' in (r/'output/static.log').read_text()
CHECK
printf 'FEREG_HOST_PROCESSORS_VALIDATION_SUCCESS job=%s snapshot=%s\n' "$JOB_ID" "$FEREG_DEV_SNAPSHOT"
