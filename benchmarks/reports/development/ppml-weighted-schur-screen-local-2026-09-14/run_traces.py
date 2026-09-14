import os
from pathlib import Path
import os,json,hashlib,copy
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import compare,named_runtime
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'baseline-identities.json').read_text());requests=json.loads((r/'baseline-route-requests.json').read_text());source=r/'baseline';assert sha(source/'dist/fereg_macos.plugin')==ids['baseline_plugin'];rows=[]
for req in requests:
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for solver in ['auto','map']:
  out=r/'trace-runs'/(req['name']+'-'+solver);manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=source,solver=solver,trace=True,identity={'artifact':ids['baseline_plugin'],'source':ids['baseline_commit'],'purpose':'Existing Schur/checkpoint traces excluded from timing screen','protocol':sha(r/'trace-protocol.md')})
  row={'name':req['name'],'solver':solver,'result':result};print(req['name'],solver,result['status'],result['seconds'],flush=True);assert result['status']=='pass'
  compare({'baseline':r/'baseline-runs'/(req['name']+'-'+solver),'candidate':out});row['exact_science']=json.loads((out/'exact-science.json').read_text());row['runtime']=named_runtime(out)
  row['lines']=[line for line in (out/'stderr.txt').read_text().splitlines() if line.startswith(('FEREG_PHASE|schur|','FEREG_SOLVER_TRACE|phase=checkpoint_lsmr_attempt|'))]
  rows.append(row);(r/'trace-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('SCHUR_TRACES_TERMINAL_PASS',flush=True)
