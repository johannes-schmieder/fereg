from pathlib import Path
import sys,json,hashlib,subprocess,base64,collections
root=Path(sys.argv[1]);mode=sys.argv[2];job=sys.argv[3];tasks=int(sys.argv[4]);old=Path(sys.argv[5])
sys.path.insert(0,str(root/'controller'))
from common import read_json,write_json,sha256,cell_id
from accounting import parse
from light_control import accept
raw=subprocess.run(['qacct','-j',job],capture_output=True,text=True)
if raw.returncode:print(raw.stderr,file=sys.stderr);sys.exit(2)
a=parse(raw.stdout,job,range(1,tasks+1));write_json(root/f'accounting/{job}.json',a)
if a['status']!='pass':print(json.dumps(a),file=sys.stderr);sys.exit(2)
plan_path=root/f'plans/{mode}.json';plan=read_json(plan_path)
assert len(plan['bundles'])==tasks
error=None
summary=dict(mode=mode,job=job,groups=[],acceptance_error=error)
counts=collections.Counter();science=collections.Counter();oracles=0
for b in plan['bundles']:
 for g in b['groups']:
  c,rep=g['cell'],g['repetition'];p=root/plan['results_directory']/cell_id(c)/f'{"cold" if rep==0 else "warm"}-{rep}'
  marker=read_json(p/'GROUP.COMPLETE.json');info=read_json(p/'input.json')
  old_info=read_json(old/'results'/cell_id(c)/f'{"cold" if rep==0 else "warm"}-{rep}'/'input.json') if mode!='smoke' else None
  exact=info==old_info if old_info else None
  if old_info and not exact:raise ValueError('Regenerated data differs from original captured input: '+cell_id(c))
  row=dict(cell=c,repetition=rep,input_sha256=info['sha256'],original_input_exact=exact,oracle_checks=marker['oracle_checks'],comparisons=marker['comparisons'],arms={},bindings={})
  for arm in marker['arm_status']:
   d=read_json(p/arm/'result.json');counts[d['status']]+=1
   row['arms'][arm]={k:d.get(k) for k in ('status','seconds','elapsed_seconds','peak_rss_gib','sample_sha256','sample_count')}
   keys=('N','rank','df_a','df_r','df_resid','df','df_k','converged','rss','deviance','ll','loglik','requested_threads','effective_threads','stata_processors','fereg_threads','fereg_requested_threads','feppml_threads','feppml_requested_threads','feppml_iterations','feppml_inner_iterations')
   row['arms'][arm]['metadata']={k:v for k,v in d.get('metadata',{}).items() if k in keys}
   for name in ['result.json','coefficients.csv','covariance.csv']:
    if (p/arm/name).exists():row['bindings'][arm+'/'+name]=sha256(p/arm/name)
  for name in ['GROUP.COMPLETE.json','input.json','oracle.json']:
   if (p/name).exists():row['bindings'][name]=sha256(p/name)
  for v in marker['comparisons']:science[v['status']]+=1
  if marker['oracle_checks'] and all(marker['oracle_checks'].values()):oracles+=1
  summary['groups'].append(row)
try:accept(root,plan_path)
except Exception as e:error=repr(e)
summary['acceptance_error']=error
summary.update(arm_status_counts=dict(counts),comparison_status_counts=dict(science),independent_oracle_groups_passed=oracles)
write_json(root/f'env/{mode}-summary.json',summary)
files=[f'accounting/{job}.json',f'plans/{mode}.json',f'env/{mode}-summary.json']
files.extend(f'bundle-results/{mode}-{i}.json' for i in range(1,tasks+1))
for name in [f'env/{mode.upper()}.ACCEPTED.json',f'env/{mode.upper()}.REJECTED.json']:
 if (root/name).exists():files.append(name)
print(json.dumps({n:dict(sha256=sha256(root/n),data=base64.b64encode((root/n).read_bytes()).decode()) for n in files}))
if error:sys.exit(1)
