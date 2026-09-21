#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_gen_mem
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
export FEREG_GIT_COMMIT=5240e58-generation-capacity3 FEREG_BUILD_DATE=2026-09-16
cpus=$(python3 harness/select-cpus.py)
qstat -j "$JOB_ID" > allocation.txt
qhost -h "$(hostname -s)" > host-context.txt
python3 - <<'CHECK'
import os,socket,subprocess,xml.etree.ElementTree as ET
host=socket.gethostname().split('.')[0]
r=ET.fromstring(subprocess.check_output(['qhost','-xml','-h',host]))
cpu=[v.text for h in r.findall('host') if h.attrib['name'].split('.')[0]==host for v in h.findall('hostvalue') if v.attrib['name']=='num_proc']
assert cpu==[os.environ['NSLOTS']],(cpu,os.environ['NSLOTS'])
CHECK
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-core --lib ppml::prepared_model::tests -- --test-threads=1) > core-model.txt 2>&1
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-core --lib ppml::prepared_separation::tests -- --test-threads=1) > core-separation.txt 2>&1
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-plugin --lib -- --test-threads=1) > ffi-tests.txt 2>&1
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-core --lib ppml::memory::tests -- --test-threads=1) > core-memory.txt 2>&1
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-core --lib ppml::result::tests -- --test-threads=1) > core-result.txt 2>&1
(cd source && taskset -c "$cpus" cargo test --locked --offline -p fereg-core --lib ppml::prepared_estimator::tests -- --test-threads=1) > core-estimator.txt 2>&1
(cd source && taskset -c "$cpus" bash scripts/build_linux.sh) > build.txt 2>&1
(cd source && FEREG_ACTION_MEMORY_AUDIT=1 taskset -c "$cpus" bash tests/mock_spi/run_actual_unix_plugin_test.sh dist/fereg_linux.plugin) > audit-loader.txt 2>&1
python3 - <<'HASH'
import hashlib,json
from pathlib import Path
art={a:hashlib.sha256((Path(p)/'dist/fereg_linux.plugin').read_bytes()).hexdigest() for a,p in [('baseline','baseline'),('candidate','source')]}
assert art['baseline']=='07bca07ac2be3c66d982ef5a5f4a3422dcdc31ef73e36a69b71a92807496211d'
Path('artifacts.json').write_text(json.dumps(art,indent=2)+'\n')
Path('build-success.txt').write_text('PUBLICATION_BUILD_PASS\n')
HASH
python3 harness/generate-publication.py
taskset -c "$cpus" python3 harness/run-screen.py > paired.txt 2>&1
printf 'PUBLICATION_COST_COMPLETE\n' > PASS
