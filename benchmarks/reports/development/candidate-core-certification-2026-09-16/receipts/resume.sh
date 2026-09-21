#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cert_tail
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export CARGO_HOME=$TOOLCHAINS/cargo RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export FEREG_GIT_COMMIT=5240e58-core-certification1 FEREG_BUILD_DATE=2026-09-16
cp -p placement.json placement-initial.json
cpus=$(python3 harness/select-cpus.py)
qstat -j "$JOB_ID" > allocation-resume.txt
qhost -h "$(hostname -s)" > host-context-resume.txt
python3 - <<'CHECK'
import os,socket,subprocess,xml.etree.ElementTree as ET
host=socket.gethostname().split('.')[0]
r=ET.fromstring(subprocess.check_output(['qhost','-xml','-h',host]))
cpu=[v.text for h in r.findall('host') if h.attrib['name'].split('.')[0]==host for v in h.findall('hostvalue') if v.attrib['name']=='num_proc']
assert cpu==[os.environ['NSLOTS']],(cpu,os.environ['NSLOTS'])
CHECK
cp placement.json placement-resume.json
python3 - <<'VERIFY'
import hashlib,json
from pathlib import Path
p=Path.cwd();i=json.loads((p/'identity.json').read_text())
for arm,key in [('source','files'),('baseline','baseline_files')]:
 for f,h in i[key].items():assert hashlib.sha256((p/arm/f).read_bytes()).hexdigest()==h,(arm,f)
assert '128 passed; 0 failed' in (p/'ffi-tests.txt').read_text()
for f,n in [('core-memory.txt',46),('core-result.txt',30),('core-estimator.txt',56)]:assert str(n)+' passed; 0 failed' in (p/f).read_text()
p.joinpath('artifact-before-wrapper-fix.json').write_text(json.dumps({'candidate':hashlib.sha256((p/'source/dist/fereg_linux.plugin').read_bytes()).hexdigest()},indent=2)+'\n')
VERIFY
cp source/dist/fereg_linux.plugin source/dist/fereg_linux.plugin-before-wrapper-fix
(cd source && taskset -c "$cpus" bash scripts/build_linux.sh) > build-recheck.txt 2>&1
(cd source && FEREG_ACTION_MEMORY_AUDIT=1 taskset -c "$cpus" bash tests/mock_spi/run_actual_unix_plugin_test.sh dist/fereg_linux.plugin) > audit-loader.txt 2>&1
python3 - <<'HASH'
import hashlib,json
from pathlib import Path
art={a:hashlib.sha256((Path(p)/'dist/fereg_linux.plugin').read_bytes()).hexdigest() for a,p in [('baseline','baseline'),('candidate','source')]}
assert art['baseline']=='c3ada630de1278c771bb7c128591ff4e9104dae3abc8e0584475598966116e83'
assert art['candidate']==json.loads(Path('artifact-before-wrapper-fix.json').read_text())['candidate']
Path('artifacts.json').write_text(json.dumps(art,indent=2)+'\n')
Path('build-success.txt').write_text('PUBLICATION_BUILD_PASS\n')
HASH
python3 harness/generate-publication.py
taskset -c "$cpus" python3 harness/run-screen.py > paired.txt 2>&1
printf 'PUBLICATION_COST_COMPLETE\n' > PASS
