import os
from pathlib import Path
import os,json,hashlib,copy
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
from compare_routes import compare_routes
r=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'baseline-identities.json').read_text());requests=json.loads((r/'baseline-route-requests.json').read_text());source=r/'baseline'
assert all(sha(source/p)==h and sha(repo/p)==h for p,h in ids['inputs'].items());assert sha(source/'dist/fereg_macos.plugin')==ids['baseline_plugin'];assert sha(repo/'dist/fereg_macos.plugin')==ids['installed_dist']
oracle=json.loads((r/'oracle-results.json').read_text());assert oracle['process_rc']==0 and oracle['terminal_rc']==['0'] and oracle['sentinel'] and len(oracle['fits'])==9
assert sha(r/'two_fe_oracle.do')==oracle['fixture_sha256']
rows=[]
for i,req in enumerate(requests):
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 row={'name':req['name']};paths={};order=['auto','map','cmg'];order=order[i:]+order[:i]
 for solver in order:
  out=r/'baseline-runs'/(req['name']+'-'+solver);manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=source,solver=solver,identity={'artifact':ids['baseline_plugin'],'source':ids['baseline_commit'],'purpose':'Existing PPML complete-route baseline screen','protocol':sha(r/'protocol.md'),'requests':sha(r/'baseline-route-requests.json')})
  row[solver]=result;paths[solver]=out;(r/'baseline-route-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(req['name'],solver,result['status'],result['seconds'],flush=True)
  if result['status']=='pass':row[solver+'_runtime']=named_runtime(out)
 if all(row[a]['status']=='pass' for a in order):
  row['science']={a:compare_routes(paths['auto'],paths[a]) for a in ['map','cmg']}
 else:row['science']='incomplete: preserve failed or timed-out route, no passing speed comparison'
 rows.append(row);(r/'baseline-route-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('SCHUR_BASELINE_ROUTES_TERMINAL_COMPLETE',flush=True)
