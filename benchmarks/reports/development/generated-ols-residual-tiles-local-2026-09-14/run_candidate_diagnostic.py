import os
from pathlib import Path
import os,sys,json,hashlib,copy
r=Path(__file__).resolve().parent;sys.path.insert(0,str(r/'harness'))
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import matrix,baseline_compare,certify
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();diag=json.loads((r/'diagnostic-built.json').read_text())['candidate'];reqs=json.loads((r/'requests.json').read_text());source=r/'diagnostic-candidate';assert all(sha(source/p)==h for p,h in diag['inputs'].items());assert sha(source/'dist/fereg_macos.plugin')==diag['plugin'];rows=[]
for req in reqs:
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm']);out=r/'candidate-diagnostic-runs'/req['name'];manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
 result=measure(r,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,manifest,source=source,trace=True,identity={'source':'25bf610 plus candidate patch','artifact':diag['plugin'],'protocol':sha(r/'followup-protocol.md'),'purpose':'Original-input generated OLS residual candidate attribution'})
 row={'name':req['name'],'result':result};(r/'candidate-diagnostic-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(req['name'],result['status'],result['seconds'],flush=True);assert result['status']=='pass'
 ref=r/'screen-runs'/(req['name']+'-candidate');row['science']=baseline_compare(out,ref);row['certificates']=certify(out);exports=[p.name for p in ref.glob('*.csv') if p.stem not in ['fereg_solver','fereg_runtime','feppml_solver','feppml_runtime']];row['exact_science']={name:(out/name).read_bytes()==(ref/name).read_bytes() for name in exports};assert all(row['exact_science'].values());cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15];assert np.array_equal(matrix(out,'fereg_solver')[:,cols],matrix(ref,'fereg_solver')[:,cols]);row['solver_fields_exact']=True
 row['phases']=[line for line in (out/'stderr.txt').read_text().splitlines() if line.startswith('FEREG_PHASE|')];assert any(line.startswith('FEREG_PHASE|ols_raw_residual|') for line in row['phases']);rows.append(row);(r/'candidate-diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n');print('\n'.join(line for line in row['phases'] if line.startswith('FEREG_PHASE|ols_raw_residual|')),flush=True)
print('GENERATED_OLS_RESIDUAL_CANDIDATE_DIAGNOSTIC_TERMINAL_PASS',flush=True)
