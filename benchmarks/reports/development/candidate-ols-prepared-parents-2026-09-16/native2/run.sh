#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ols_parent
#$ -cwd
#$ -j y
set -euo pipefail
export CARGO_HOME=$RUST_TOOLCHAINS/cargo
export RUSTUP_HOME=$RUST_TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export FEREG_GIT_COMMIT=5240e58-ols-prepared-parent2
export FEREG_BUILD_DATE=2026-09-16
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1
cd source
cpus=$(python3 -c 'import os; print(",".join(map(str,sorted(os.sched_getaffinity(0))[:int(os.environ["NSLOTS"])])))')
taskset -c "$cpus" cargo test --locked --offline --workspace --lib --no-run --message-format=json > ../compiler-artifacts.jsonl
taskset -c "$cpus" cargo test --locked --offline -p fereg-core --test feppml_constrained_memory_contract --no-run --message-format=json >> ../compiler-artifacts.jsonl
python3 - <<'HASH' > ../test-artifacts.json
from pathlib import Path
import json,hashlib
artifacts=[]
for line in Path('../compiler-artifacts.jsonl').read_text().splitlines():
 record=json.loads(line)
 if record.get('reason')=='compiler-artifact' and record.get('executable'):
  p=Path(record['executable'])
  artifacts.append(dict(package_id=record['package_id'],target=record['target'],profile=record['profile'],path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest(),bytes=p.stat().st_size))
assert len(artifacts)==4,artifacts
print(json.dumps(artifacts,indent=2))
HASH
python3 - "$cpus" <<'RUN'
from pathlib import Path
import json,hashlib,subprocess,sys
artifacts=json.loads(Path('../test-artifacts.json').read_text())
failed=[]
for a in sorted(artifacts,key=lambda a:a['target']['name']):
 p=Path(a['path']);assert hashlib.sha256(p.read_bytes()).hexdigest()==a['sha256']
 print('EXACT_ARTIFACT '+a['target']['name']+' '+a['sha256'],flush=True)
 filters = ['ols::prepared::tests::', 'categorical', 'memory::', 'recovery', 'constrained'] if a['target']['name'] == 'fereg_core' else ['']
 for selected in filters:
  rc=subprocess.call(['taskset','-c',sys.argv[1],str(p),selected,'--test-threads=1'])
  if rc: failed.append([a['target']['name'],selected,rc])
print('SCOPE_FAILURES',json.dumps(failed),flush=True)
if failed: raise SystemExit(1)
print('OLS_PREPARED_PARENT_PASS',flush=True)
RUN
