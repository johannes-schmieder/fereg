from pathlib import Path
import csv,hashlib,json,os,re,subprocess,time
import numpy as np
import pandas as pd
from audit_pragmatic_memory import audit_case
root=Path(__file__).resolve().parent
source=Path(os.environ['SOURCE_ROOT'])
prior=Path('${DEV_ROOT}/20260915T102313Z-german5-0660945')
data=prior/'input/sample.dta'
old=prior/'comparison-ols-isolated-1/ols-native-1'
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(8<<20),b''):h.update(b)
 return h.hexdigest()
assert sha(source/'dist/fereg_linux.plugin')=='bb1eb0cfc8f44889b8ad858bc4adcc2b189e7dc378501b5328ab6784a1e68018'
assert sha(data)=='6498d6051aac8cce1ff545c1a0228f655fcf7d6e88845b0c6f1ce564e6e7f59e'
out=root/'native';out.mkdir()
for name in ['empty-plus','empty-personal','compiled','scratch']:(out/name).mkdir()
(out/'run.do').write_text(f'''capture noisily do "{root}/fit.do" "{source}" "{data}" "{out}" ols native 4 "{prior}/references"
local rc=_rc
file open result using "{out}/result.txt", write replace
file write result "STATA_RC=`rc'" _n
file close result
exit `rc'
''')
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_') and k not in ('PYTHONPATH','LD_PRELOAD')}
env.update(TMPDIR=str(out/'scratch'),OMP_NUM_THREADS='4',OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',NUMEXPR_NUM_THREADS='1',RAYON_NUM_THREADS='4',FEREG_MEMORY_LIMIT_BYTES='42949672960',FEREG_ACTION_MEMORY_AUDIT='1')
start=time.monotonic()
with (out/'stdout.txt').open('w') as stdout,(out/'stderr.txt').open('w') as stderr:
 fit=subprocess.run(['/usr/bin/time','-v','stata-mp','-b','do','run.do'],cwd=out,env=env,stdout=stdout,stderr=stderr,timeout=900)
r=dict(source_manifest_sha256=sha(source.parent/'source.sha256'),artifact_sha256=sha(source/'dist/fereg_linux.plugin'),input_sha256=sha(data),reference_lock_sha256=sha(source/'references.lock.toml'),process_rc=fit.returncode,process_seconds=time.monotonic()-start,job_id=os.environ['JOB_ID'],host=os.uname().nodename,slots=int(os.environ['NSLOTS']),threads=4,native_limit_bytes=42949672960,science_pass=False,whole_action_accepted=False,no_reference_estimation=True,no_qualification=True)
def save():(root/'validation.json').write_text(json.dumps(r,indent=2)+'\n')
save()
assert fit.returncode==0
assert (out/'result.txt').read_text().strip()=='STATA_RC=0'
assert (out/'fit-result.txt').read_text().startswith('FIT_RC=0\n')
assert 'GERMAN_FIVE_PERCENT_FIT_COMPLETE|family=ols|arm=native' in (out/'run.log').read_text()
tables=['b','V','fereg_status','fereg_fit','fereg_dof','fereg_fe','fereg_certificate']
# Check each outcome independently and preserve discrepancies without reruns.
comparisons={name:(out/(name+'.tsv')).read_bytes()==(old/(name+'.tsv')).read_bytes() for name in tables}
left=pd.read_stata(old/'outputs.dta',convert_categoricals=False)
right=pd.read_stata(out/'outputs.dta',convert_categoricals=False)
assert left.columns.equals(right.columns)
columns={name:np.array_equal(left[name].to_numpy(),right[name].to_numpy(),equal_nan=True) for name in left.columns}
r.update(exact_scientific_tables=comparisons,saved_columns_exact=columns,retained_rows=int(right['sample'].sum()),sample_order_exact=columns['sample'] and columns['obs_id'])
del left,right
with (out/'fereg_memory.tsv').open() as f:matrix=list(csv.reader(f,delimiter='\t'))
with (out/'fereg_memory.csv').open('w') as f:
 w=csv.writer(f);w.writerow(['row','column','value'])
 for i,row in enumerate(matrix[1:],1):
  for j,value in enumerate(row[1:],1):w.writerow([i,j,value.strip()])
r['memory_audit']=audit_case(out)
r['stata_memory_observation']={k:float(v) for k,v in csv.reader((out/'stata-memory.tsv').open(),delimiter='\t')}
r['command_seconds']=float(re.search(r'COMMAND_SECONDS=(\S+)',(out/'fit-result.txt').read_text())[1])
r['process_peak_rss_bytes']=int(re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)',(out/'stderr.txt').read_text())[1])*1024
r['output_hashes']={p.name:sha(p) for p in out.glob('*.tsv')}
r['raw_log_hashes']={n:sha(out/n) for n in ['run.log','stderr.txt','fit-result.txt','result.txt']}
r['harness_hashes']={p.name:sha(p) for p in root.iterdir() if p.suffix in ('.py','.do','.sh')}
r['science_pass']=all(comparisons.values()) and all(columns.values()) and r['retained_rows']==4361733
save()
assert r['science_pass'],'preserved scientific comparison failed; no rerun'
print('GERMAN_OLS_EXPECTED_HEAP_OBSERVATION_COMPLETE',flush=True)
