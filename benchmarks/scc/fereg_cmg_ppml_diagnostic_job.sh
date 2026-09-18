#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cmg_diag
#$ -pe omp 28
#$ -l h_rt=00:10:00
#$ -l mem_per_core=3G
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${PAIR_ROOT:?}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/cmg-ppml-diagnostic-[a-zA-Z0-9._-]+$ ]]
[[ "$PAIR_ROOT" =~ ^/projectnb/welfgr/fereg/dev/cmg-integration-pairs-[a-zA-Z0-9._-]+$ ]]
[[ "${NSLOTS:-}" == 28 ]]
cd "$RUN_ROOT"
test ! -e DONE
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export OMP_NUM_THREADS=4 RAYON_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
sha256sum -c diagnostic.sha256
cpus=$(python3 "$PAIR_ROOT/harness/select-cpus.py")
qstat -j "$JOB_ID" > allocation.txt
# Same binaries and input, three diagnostic pairs plus one separately labelled
# traced pair. Preserve the original untraced performance flag regardless.
taskset -c "$cpus" python3 - <<'PY'
from pathlib import Path
import os,json,hashlib,sys,subprocess,statistics
root=Path.cwd();original=Path(os.environ['PAIR_ROOT']);sys.path.insert(0,str(original/'harness'))
from measure import measure
from common import sha256,write_json
assert 'FEREG_CMG_PAIRS_COMPLETE' in (original/'PASS').read_text()
art=json.loads((original/'artifacts.json').read_text());ident=json.loads((original/'identity.json').read_text())
for a,p in [('baseline','baseline'),('candidate','source')]:assert sha256(original/p/'dist/fereg_linux.plugin')==art[a]
req=[r for r in json.loads((original/'requests.json').read_text())if r['id']=='cmg_ppml'][0]
bound=json.loads((original/'generated-bindings.json').read_text())[req['id']];data=original/'data'/req['data']
assert sha256(data)==bound['data_sha256']
# Run in the new output directory and preserve the measurement harness verbatim.
(root/'work/refs').mkdir(parents=True,exist_ok=True)
import socket,xml.etree.ElementTree as ET
host=socket.gethostname().split('.')[0]
def isolation():
 tree=ET.fromstring(subprocess.check_output(['qstat','-xml','-u','*']))
 jobs=[(j.findtext('JB_job_number'),j.findtext('slots'))for j in tree.findall('.//job_list')if j.findtext('queue_name','').split('@')[-1].split('.')[0]==host]
 assert jobs==[(os.environ['JOB_ID'],'28')],jobs
 return jobs
rows=[];stata=os.environ['STATA_MP_PATH']
for pair,order in enumerate([['baseline','candidate'],['candidate','baseline'],['baseline','candidate'],['candidate','baseline']],1):
 trace=pair==4
 if trace:
  wrapper=root/'trace-stata.sh';wrapper.write_text('#!/bin/bash\nexport FEREG_SOLVER_TRACE=1\nexec '+__import__('shlex').quote(stata)+' "$@"\n');wrapper.chmod(0o755);os.environ['STATA_MP_PATH']=str(wrapper)
 for arm in order:
  occupants=isolation();dest=root/'runs'/('p%d-%s'%(pair,arm))
  r=measure(root,req['cell'],'feppml','cold',data,data,dest,{'measurement':{'startup_timeout_seconds':90,'command_timeout_seconds':180,'rss_interval_seconds':.01}},source=original/('baseline'if arm=='baseline'else'source'),solver='cmg',identity={'diagnostic':True,'traced':trace,'artifact_sha256':art[arm],'source':ident['baseline_identity']if arm=='baseline'else ident['label']})
  isolation();assert r['status']=='pass',r
  trace_lines=[s for f in ['stdout.txt','stderr.txt'] for s in (dest/f).read_text(errors='replace').splitlines()if s.startswith('FEREG_CMG_')]
  rows.append({'pair':pair,'arm':arm,'traced':trace,'result':r,'trace':trace_lines,'occupants':occupants})
  write_json(root/'diagnostic.json',rows)
  print(pair,arm,'traced'if trace else'untraced',r['seconds'],flush=True)
# Check scientific values against the original matching arm; only time fields
# and runtime/resource diagnostics are excluded from these exact comparisons.
for row in rows:
 dest=root/'runs'/('p%d-%s'%(row['pair'],row['arm']));ref=original/'runs'/('cmg_ppml-p1-'+row['arm'])
 for name in ['b','V','feppml_status','feppml_fit','feppml_dof','feppml_fe','feppml_certificate','saved_outputs']:
  assert (dest/(name+'.csv')).read_bytes()==(ref/(name+'.csv')).read_bytes(),name
 assert row['result']['sample_sha256']==json.loads((ref/'result.json').read_text())['sample_sha256']
write_json(root/'summary.json',{'science_exact_to_original_arm':True,'original_flag_preserved':True,'untraced_median_seconds':{a:statistics.median(r['result']['seconds']for r in rows if r['arm']==a and not r['traced'])for a in ['baseline','candidate']},'artifacts':art})
PY
printf 'FEREG_CMG_PPML_DIAGNOSTIC_PASS job=%s\n' "$JOB_ID" > DONE
cat DONE
