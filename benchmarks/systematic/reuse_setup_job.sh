#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_sweep_setup
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${REFERENCE_RUN:?}" "${ORIGIN_SOURCE:?}" "${FEREG_SCC_RUNS_ROOT:?}"
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19 R/4.5.2
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export MPLCONFIGDIR="$TMPDIR/matplotlib"
test ! -e "$RUN_ROOT/env/SETUP.SUCCESS"
python3 - "$RUN_ROOT" "$REFERENCE_RUN" "$ORIGIN_SOURCE" <<'PY'
import hashlib,json,pathlib,shutil,sys,tarfile
root,old,origin=map(pathlib.Path,sys.argv[1:])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
read=lambda p:json.loads(p.read_text())
def write(p,v):p.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
for name,digest in read(root/'input/deployment.json').items():
    assert sha(root/name)==digest,name
ident=read(root/'input/candidate.json');source=read(root/'input/source.json')
validated=read(root/'input/linux-validation.json')
assert validated['status']=='success' and validated['tests_failed']==0
assert source['snapshot']==validated['snapshot']
assert sha(root/'input/candidate.tar.gz')==ident['sha256']
for name,digest in source['files'].items():assert sha(origin/name)==digest,name
assert sha(origin/'dist/fereg_linux.plugin')==validated['plugin_sha256']
(root/'env').mkdir(exist_ok=True)
frozen=read(old/'env/frozen.json')
checked={}
for name,digest in frozen['runtime'].items():
    if name.startswith(('references/','env/R/fixest/')) or '/site-packages/pyfixest/' in name:
        assert sha(old/name)==digest,name
        checked[name]=digest
write(root/'env/reference-origin.json',dict(fingerprint=frozen['fingerprint'],files=checked))
# The pinned comparator environment is immutable and already compute-validated.
# Link its runtime rather than duplicating a complete environment on projectnb.
for name in ['references','env/R','env/python']:
    (root/name).symlink_to(old/name,target_is_directory=True)
for name in ['python-freeze.txt','R-packages.csv','reference-files.json']:
    shutil.copyfile(old/'env'/name,root/'env'/name)
(root/'candidate').mkdir()
with tarfile.open(root/'input/candidate.tar.gz') as archive:
    archive.extractall(root/'candidate',filter='data')
for name,digest in source['files'].items():assert sha(root/'candidate'/name)==digest,name
(root/'candidate/dist').mkdir(exist_ok=True)
shutil.copyfile(origin/'dist/fereg_linux.plugin',root/'candidate/dist/fereg_linux.plugin')
write(root/'env/artifact-origin.json',dict(source_snapshot=source['snapshot'],candidate_commit=ident['commit'],
    origin=str(origin),plugin_sha256=validated['plugin_sha256'],linux_validation_sha256=sha(root/'input/linux-validation.json')))
names=['candidate/dist/fereg_linux.plugin','env/python-freeze.txt','env/R-packages.csv','env/reference-files.json']
names += ['candidate/'+name for name in source['files']]
write(root/'env/setup.json',{name:sha(root/name) for name in names})
PY
cd "$RUN_ROOT/controller"
"$RUN_ROOT/env/python/bin/python" -m unittest -v test_campaign test_evidence test_light test_fixest_sweep test_worker_diagnostics > "$RUN_ROOT/logs/controller-tests.txt" 2>&1
"$RUN_ROOT/env/python/bin/python" "$RUN_ROOT/controller/provenance.py" freeze "$RUN_ROOT" > "$RUN_ROOT/env/fingerprint.txt"
"$RUN_ROOT/env/python/bin/python" "$RUN_ROOT/controller/campaign.py" smoke "$RUN_ROOT" "$RUN_ROOT/plans/smoke.json"
printf 'SYSTEMATIC_SETUP_SUCCESS job=%s\n' "$JOB_ID" > "$RUN_ROOT/env/SETUP.SUCCESS"
cat "$RUN_ROOT/env/SETUP.SUCCESS"
