import os
from pathlib import Path
import os,sys,json,hashlib,copy
r=Path(__file__).resolve().parent;sys.path.insert(0,str(r/'harness'))
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import matrix,baseline_compare,certify
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'baseline-identities.json').read_text());diag=json.loads((r/'diagnostic-built.json').read_text())['baseline'];reqs=json.loads((r/'requests.json').read_text());harness=json.loads((r/'harness-identities.json').read_text());repo=Path(os.environ['REPOSITORY_ROOT'])
assert all(sha(repo/p)==h and sha(r/'baseline'/p)==h for p,h in ids['baseline_inputs'].items());assert sha(repo/'dist/fereg_macos.plugin')==ids['installed_dist']
assert all(sha(r/'diagnostic-baseline'/p)==h for p,h in diag['inputs'].items());assert sha(r/'baseline/dist/fereg_macos.plugin')==ids['baseline_plugin'];assert sha(r/'diagnostic-baseline/dist/fereg_macos.plugin')==diag['plugin']
assert all(sha(r/'harness'/p)==h for p,h in harness.items())
rows=[]
for req in reqs:
 paths={};row={'name':req['name']}
 for arm in ['baseline','diagnostic-baseline']:
  assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm']);out=r/'runs'/(req['name']+'-'+arm);paths[arm]=out;manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,manifest,source=r/arm,trace=arm.startswith('diagnostic'),identity={'source':'25bf610','artifact':diag['plugin'] if arm.startswith('diagnostic') else ids['baseline_plugin'],'protocol':sha(r/'protocol.md'),'purpose':'Original-input generated OLS residual cost audit'})
  row[arm]=result;(r/'diagnostic-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(req['name'],arm,result['status'],result['seconds'],flush=True);assert result['status']=='pass'
 a,b=paths['baseline'],paths['diagnostic-baseline'];row['science']=baseline_compare(b,a);row['certificates']={k:certify(p) for k,p in paths.items()};exports=[p.name for p in a.glob('*.csv') if p.stem not in ['fereg_solver','fereg_runtime','feppml_solver','feppml_runtime']];row['exact_science']={name:(a/name).read_bytes()==(b/name).read_bytes() for name in exports};assert all(row['exact_science'].values());cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15];assert np.array_equal(matrix(a,'fereg_solver')[:,cols],matrix(b,'fereg_solver')[:,cols]);row['solver_fields_exact']=True
 row['phases']=[line for line in (b/'stderr.txt').read_text().splitlines() if line.startswith('FEREG_PHASE|')];assert any(line.startswith('FEREG_PHASE|ols_raw_residual|') for line in row['phases']);rows.append(row);(r/'diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n');print('\n'.join(line for line in row['phases'] if line.startswith('FEREG_PHASE|ols_raw_residual|')),flush=True)
print('GENERATED_OLS_RESIDUAL_DIAGNOSTIC_TERMINAL_PASS',flush=True)
