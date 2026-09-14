import os
from pathlib import Path
import os,json,hashlib,copy,sys
r=Path(__file__).resolve().parent;sys.path.insert(0,str(r/'harness'))
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
from final_compare import baseline_compare,certify,matrix
from compare_routes import compare_routes
import numpy as np
repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());requests=json.loads((r/'requests.json').read_text())
assert all(sha(r/'baseline'/p)==h for p,h in ids['baseline_inputs'].items())
assert all(sha(r/'candidate'/p)==h and sha(repo/p)==h for p,h in ids['candidate_inputs'].items())
assert sha(repo/'dist/fereg_macos.plugin')==ids['installed_dist']
for arm in ['baseline','candidate']:assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
oracles=json.loads((r/'oracle-results.json').read_text());assert len(oracles)==2 and all(x['process_rc']==0 and x['terminal_rc']==['0'] and x['sentinel'] and len(x['fits'])==5 for x in oracles)
harness=json.loads((r/'harness-identities.json').read_text());assert all(sha(r/'harness'/p)==h for p,h in harness.items())
rows=[]
for i,req in enumerate(requests):
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 row={'name':req['name']};paths={};order=['candidate','baseline'] if i%2==0 else ['baseline','candidate'];ppml=req['cell']['model']=='ppml'
 for arm in order:
  out=r/'screen-runs'/(req['name']+'-'+arm);manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,req['cell'],'feppml' if ppml else 'fereg','warm',Path(req['data']),Path(req['warm']),out,manifest,source=r/arm,solver='auto',identity={'artifact':ids[arm+'_plugin'],'baseline_source':ids['baseline_commit'],'purpose':'Generated predictor complete-command screen','protocol':sha(r/'protocol.md'),'requests':sha(r/'requests.json'),'worker':harness['stata_worker.do']})
  row[arm]=result;paths[arm]=out;(r/'screen-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(req['name'],arm,result['status'],result['seconds'],flush=True)
  if result['status']=='pass' and ppml:row[arm+'_runtime']=named_runtime(out)
 if all(row[a]['status']=='pass' for a in order):
  if ppml: row['science']=compare_routes(paths['baseline'],paths['candidate'])
  else:
   row['science']=baseline_compare(paths['candidate'],paths['baseline']);row['certificates']={a:certify(p) for a,p in paths.items()}
   exact=['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset'];row['exact_exports']={name:(paths['candidate']/(name+'.csv')).read_bytes()==(paths['baseline']/(name+'.csv')).read_bytes() for name in exact};assert all(row['exact_exports'].values()),row['exact_exports']
   cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15];assert np.array_equal(matrix(paths['candidate'],'fereg_solver')[:,cols],matrix(paths['baseline'],'fereg_solver')[:,cols]);row['solver_fields_exact']=True
 else:row['science']='incomplete: preserve every failure or timeout, no passing speed claim'
 rows.append(row);(r/'screen-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('GENERATED_PREDICTOR_SCREEN_TERMINAL_COMPLETE',flush=True)
