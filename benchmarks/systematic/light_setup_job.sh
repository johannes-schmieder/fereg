#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_light_setup
#$ -pe omp 4
#$ -l mem_per_core=4G
#$ -l h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${REFERENCE_RUN:?}" "${FEREG_SCC_RUNS_ROOT:?}" "${FEREG_SCC_RUST_ROOT:?}"
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
mkdir -p "$RUN_ROOT/env" "$RUN_ROOT/logs"
test ! -e "$RUN_ROOT/env/SETUP.SUCCESS"
python3 - "$RUN_ROOT" "$REFERENCE_RUN" <<'PY'
import hashlib,json,pathlib,shutil,sys,tarfile
root,old=map(pathlib.Path,sys.argv[1:])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ident=json.loads((root/'input/candidate.json').read_text())
assert sha(root/'input/candidate.tar.gz')==ident['sha256']
frozen=json.loads((old/'env/frozen.json').read_text())
checked={}
for name,h in frozen['runtime'].items():
    if name.startswith(('references/','env/R/fixest/')) or '/site-packages/pyfixest/' in name:
        assert sha(old/name)==h,name
        checked[name]=h
(root/'env/reference-origin.json').write_text(json.dumps({'fingerprint':frozen['fingerprint'],'files':checked},indent=2)+'\n')
for name in ['references','env/R','env/python']:
    shutil.copytree(old/name,root/name,symlinks=True)
for name in ['python-freeze.txt','R-packages.csv','reference-files.json']:
    shutil.copyfile(old/'env'/name,root/'env'/name)
shutil.copytree(old/'input/archives',root/'input/archives')
(root/'candidate').mkdir()
with tarfile.open(root/'input/candidate.tar.gz') as t:t.extractall(root/'candidate',filter='data')
PY
export CARGO_HOME="$FEREG_SCC_RUST_ROOT/cargo" RUSTUP_HOME="$FEREG_SCC_RUST_ROOT/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$RUN_ROOT/env/candidate-target" CARGO_BUILD_JOBS=4
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export FEREG_GIT_COMMIT="$(python3 -c 'import json,os; print(json.load(open(os.environ["RUN_ROOT"]+"/input/candidate.json"))["commit"])')"
export FEREG_BUILD_DATE=2026-09-26
export OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1
cd "$RUN_ROOT/candidate"
cargo fetch --locked > "$RUN_ROOT/logs/fetch.txt" 2>&1
export CARGO_NET_OFFLINE=true
bash scripts/build_linux.sh > "$RUN_ROOT/logs/candidate-build.txt" 2>&1
python3 - "$RUN_ROOT" <<'PY'
import hashlib,json,pathlib,sys
r=pathlib.Path(sys.argv[1]);names=['candidate/dist/fereg_linux.plugin','env/python-freeze.txt','env/R-packages.csv','env/reference-files.json']
(r/'env/setup.json').write_text(json.dumps({p:hashlib.sha256((r/p).read_bytes()).hexdigest() for p in names},indent=2)+'\n')
PY
printf 'SYSTEMATIC_SETUP_SUCCESS job=%s\n' "$JOB_ID" > "$RUN_ROOT/env/SETUP.SUCCESS"
cat "$RUN_ROOT/env/SETUP.SUCCESS"
