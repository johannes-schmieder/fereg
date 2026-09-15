#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_fixed_fixtures
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export RAYON_NUM_THREADS=16 OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
python3 - <<'RUN'
from pathlib import Path
import json,hashlib,subprocess,shutil
root=Path.cwd();summary=json.loads((root/'summary.json').read_text());assert summary['all_science_exact']
m=json.loads((root/'fixed-cases.json').read_text());fixtures=[]
for f in m['functional_fixtures']:
 p=root/'source'/f['path'];assert hashlib.sha256(p.read_bytes()).hexdigest()==f['sha256'];fixtures.append(str(p))
plugin=root/'source/dist/fereg_linux.plugin';assert hashlib.sha256(plugin.read_bytes()).hexdigest()==summary['artifacts']['candidate']
subprocess.check_call(['python3',str(root/'source/scripts/run_stata_tests.py'),'--stata',shutil.which('stata-mp'),'--plugin',str(plugin),'--plugin-sha256',summary['artifacts']['candidate'],'--log-dir',str(root/'fixture-logs'),'--summary-json',str(root/'fixture-summary.json')]+fixtures,cwd=root/'source')
print('FIXED_LINUX_FIXTURES_PASS',flush=True)
RUN
