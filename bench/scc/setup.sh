#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_bench_setup
#$ -pe omp 28
#$ -l num_proc=28
#$ -l mem_per_core=3G
#$ -l h_rt=01:00:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${REFERENCE_ROOT:?}"
cd "$RUN_ROOT"
sha256sum -c input/SHA256SUMS
mkdir source environment
tar -xf input/source.tar -C source
cp input/source.json source/.fereg-source.json
cp -a "$REFERENCE_ROOT/environment/references" environment/
cp -a "$REFERENCE_ROOT/environment/r-library" environment/
cp "$REFERENCE_ROOT/environment/environment.json" environment/reference-origin.json
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
python3 - "$RUN_ROOT" "$REFERENCE_ROOT" <<'PY'
import hashlib,pathlib,shutil,sys,tomllib
r,origin=map(pathlib.Path,sys.argv[1:]);refs=r/'environment/references'
for ref in tomllib.loads((r/'source/references.lock.toml').read_text())['reference']:
    if ref['name'] not in ['ftools','reghdfe','ppmlhdfe','require']:continue
    for name,expected in ref.get('required_capture_sha256',{}).items():
        path=refs/pathlib.Path(name).name
        if not path.exists():
            shutil.copy2(origin/'environment/sources'/ref['name']/name,path)
        assert hashlib.sha256(path.read_bytes()).hexdigest()==expected,name
PY
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=false CARGO_BUILD_JOBS=8
export PATH="$CARGO_HOME/bin:$PATH"
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1
export R_LIBS_USER="$RUN_ROOT/environment/r-library"
export FEREG_GIT_COMMIT
FEREG_GIT_COMMIT=$(python3 -c 'import json;print(json.load(open("input/source.json"))["commit"])')
cd source
cargo fetch --locked --target x86_64-unknown-linux-gnu > "$RUN_ROOT/logs/dependency-fetch.log" 2>&1
export CARGO_NET_OFFLINE=true
bash scripts/build_linux.sh > "$RUN_ROOT/logs/build.log" 2>&1
python3 bench/build_clock.py --output "$RUN_ROOT/environment/command-clock.plugin" > "$RUN_ROOT/logs/clock-build.log" 2>&1
python3 -m unittest discover -s bench/tests > "$RUN_ROOT/logs/harness-tests.log" 2>&1
cd "$RUN_ROOT/environment/references"
stata-mp -b do "$RUN_ROOT/source/bench/scc/prepare_references.do" "$RUN_ROOT/environment/references"
mv prepare_references.log "$RUN_ROOT/logs/reference-prepare.log"
python3 - "$RUN_ROOT/logs/reference-prepare.log" <<'PY'
import pathlib,sys
assert 'CAMPAIGN_REFERENCES_COMPLETE rc=0' in pathlib.Path(sys.argv[1]).read_text()
PY
cd "$RUN_ROOT/source"
python3 bench/campaign.py prepare "$RUN_ROOT"
Rscript -e 'library(fixest);library(foreign);library(jsonlite);library(digest);writeLines(toJSON(as.list(setNames(vapply(loadedNamespaces(),find.package,character(1)),loadedNamespaces())),auto_unbox=TRUE),file.path(Sys.getenv("RUN_ROOT"),"environment/r-package-paths.json"))'
python3 - "$RUN_ROOT" <<'PY'
import hashlib,json,pathlib,sys,tomllib,shutil
r=pathlib.Path(sys.argv[1]);refs=r/'environment/references'
lock=tomllib.loads((r/'source/references.lock.toml').read_text())
for ref in lock['reference']:
    if ref['name'] not in ['ftools','reghdfe','ppmlhdfe','require']:continue
    for name,expected in ref.get('required_capture_sha256',{}).items():
        path=refs/pathlib.Path(name).name
        assert hashlib.sha256(path.read_bytes()).hexdigest()==expected,name
paths=[r/'source/dist/fereg_linux.plugin',r/'environment/command-clock.plugin',
       *sorted((r/'source/stata').glob('*')),
       *sorted(refs.rglob('*')),*sorted((r/'environment/r-library').rglob('*')),
       *sorted((r/'source/bench').rglob('*')),r/'manifest.json']
for folder in json.loads((r/'environment/r-package-paths.json').read_text()).values():
    paths.extend(sorted(pathlib.Path(folder).rglob('*')))
paths.extend(pathlib.Path(shutil.which(name)).resolve() for name in ['stata-mp','Rscript'])
def key(p):
    try:return str(p.relative_to(r))
    except ValueError:return str(p)
inventory={key(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file()}
(r/'environment/runtime.json').write_text(json.dumps(inventory,indent=2)+'\n')
PY
printf 'CAMPAIGN_SETUP_COMPLETE\n' > "$RUN_ROOT/environment/COMPLETE"
