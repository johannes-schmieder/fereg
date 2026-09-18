#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cmg_pairs
#$ -pe omp 28
#$ -l h_rt=00:30:00
#$ -l mem_per_core=3G
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${SOURCE_SHA:?}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/cmg-integration-pairs-[a-zA-Z0-9._-]+$ ]]
[[ "$SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]
[[ "${NSLOTS:-}" == 28 ]]
cd "$RUN_ROOT"
test ! -e PASS
sha256sum -c source.sha256 > source-before.txt
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$RUN_ROOT/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export FEREG_GIT_COMMIT="${SOURCE_SHA:0:8}-preserved-memory-overlay" FEREG_BUILD_DATE=2026-09-17
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
# The baseline artifact is preserved, hash-verified, and bound to the prior source snapshot.
(cd source && cargo test --locked --offline -p fereg-core --lib fe::cmg -- --test-threads=1) > cmg-tests.txt 2>&1
(cd source && cargo test --locked --offline -p fereg-plugin --lib -- --test-threads=1) > plugin-tests.txt 2>&1
(cd source && taskset -c "$cpus" bash scripts/build_linux.sh) > build.txt 2>&1
(cd source && FEREG_ACTION_MEMORY_AUDIT=1 taskset -c "$cpus" bash tests/mock_spi/run_actual_unix_plugin_test.sh dist/fereg_linux.plugin) > loader.txt 2>&1
python3 - <<'HASH'
import hashlib,json
from pathlib import Path
art={a:hashlib.sha256((Path(p)/'dist/fereg_linux.plugin').read_bytes()).hexdigest() for a,p in [('baseline','baseline'),('candidate','source')]}
identity=json.loads(Path('identity.json').read_text())
assert art['baseline']==identity['baseline_artifact_sha256']
Path('artifacts.json').write_text(json.dumps(art,indent=2)+'\n')
Path('build-success.txt').write_text('PUBLICATION_BUILD_PASS\n')
HASH
python3 harness/generate-publication.py
taskset -c "$cpus" python3 harness/run-screen.py > paired.txt 2>&1
sha256sum -c source.sha256 > source-after.txt
printf 'FEREG_CMG_PAIRS_COMPLETE source=%s job=%s\n' "$SOURCE_SHA" "$JOB_ID" > PASS
cat PASS
