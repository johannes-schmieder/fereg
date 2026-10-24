#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_veneto_setup
#$ -pe omp 4
#$ -l mem_per_core=3G
#$ -l h_rt=00:30:00
#$ -j y
set -euo pipefail
umask 077
: "${RUN_ROOT:?}" "${REFERENCE_ROOT:?}"
cd "$RUN_ROOT"
sha256sum -c input/SHA256SUMS
mkdir source environment
tar -xf input/source.tar -C source
cp input/source.json source/.fereg-source.json
# Reuse qualified pinned comparators and the monotonic clock. These are frozen
# below; no builds or package installation take place on a measured host.
for directory in references r-library; do
    ln -s "$REFERENCE_ROOT/environment/$directory" "environment/$directory"
done
cp "$REFERENCE_ROOT/environment/command-clock.plugin" environment/
cp "$REFERENCE_ROOT/environment/reference-origin.json" environment/
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 OMP_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1 MPLCONFIGDIR="$RUN_ROOT/environment/matplotlib"
export R_LIBS_USER="$RUN_ROOT/environment/r-library"
cd source
python3 -m unittest discover -s bench/tests > "$RUN_ROOT/logs/harness-tests.log" 2>&1
python3 scripts/check_github_install.py > "$RUN_ROOT/logs/package-check.log" 2>&1
python3 bench/panel_campaign.py audit "$RUN_ROOT" > "$RUN_ROOT/logs/input-audit.log" 2>&1
Rscript -e 'library(fixest);library(foreign);library(jsonlite);library(digest);stopifnot(as.character(packageVersion("fixest"))=="0.14.2");writeLines(toJSON(as.list(setNames(vapply(loadedNamespaces(),find.package,character(1)),loadedNamespaces())),auto_unbox=TRUE),file.path(Sys.getenv("RUN_ROOT"),"environment/r-package-paths.json"))'
python3 - "$RUN_ROOT" <<'PY'
import hashlib,json,pathlib,shutil,sys,tomllib
r=pathlib.Path(sys.argv[1]);refs=r/'environment/references'
for ref in tomllib.loads((r/'source/references.lock.toml').read_text())['reference']:
    if ref['name'] not in ['ftools','reghdfe','ppmlhdfe','require']:continue
    for name,expected in ref.get('required_capture_sha256',{}).items():
        assert hashlib.sha256((refs/pathlib.Path(name).name).read_bytes()).hexdigest()==expected,name
paths=[r/'source/dist/fereg_linux.plugin',r/'environment/command-clock.plugin',r/'manifest.json',
       *sorted((r/'source/stata').glob('*')),*sorted((r/'source/bench').rglob('*')),
       *sorted(refs.rglob('*')),*sorted((r/'environment/r-library').rglob('*')),
       *sorted((r/'data').glob('*')),*sorted((r/'smoke-data').glob('*'))]
for folder in json.loads((r/'environment/r-package-paths.json').read_text()).values():
    paths.extend(sorted(pathlib.Path(folder).rglob('*')))
paths.extend(pathlib.Path(shutil.which(n)).resolve() for n in ['stata-mp','Rscript'])
def key(p):
    try:return str(p.relative_to(r))
    except ValueError:return str(p)
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(1<<20),b''):h.update(b)
    return h.hexdigest()
(r/'environment/runtime.json').write_text(json.dumps({key(p):sha(p) for p in paths if p.is_file()},indent=2)+'\n')
PY
printf 'PANEL_SETUP_COMPLETE\n' > "$RUN_ROOT/environment/COMPLETE"
printf 'PANEL_SETUP_COMPLETE job=%s\n' "$JOB_ID"
