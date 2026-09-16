#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_mem_rest
#$ -cwd
#$ -j y
set -euo pipefail
export RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1
python3 - <<'PY'
import json,hashlib,subprocess,os
from pathlib import Path
cpus=','.join(map(str,sorted(os.sched_getaffinity(0))[:int(os.environ['NSLOTS'])]))
failed=[]
for a in json.loads(Path('test-artifacts.json').read_text()):
 name=a['target']['name']
 if name=='feppml_constrained_memory_contract':continue
 p=Path(a['path']);assert hashlib.sha256(p.read_bytes()).hexdigest()==a['sha256']
 filters=['memory::','ppml::','certified_linear_session','retained_fresh_frozen','strict_candidate'] if name=='fereg_core' else ['']
 for selected in filters:
  print('EXACT_ARTIFACT',name,a['sha256'],selected,flush=True)
  rc=subprocess.call(['taskset','-c',cpus,str(p),selected,'--test-threads=1'])
  if rc:failed.append([name,selected,rc])
print('REMAINING_SCOPE_RESULTS',json.dumps(failed),flush=True)
raise SystemExit(bool(failed))
PY
