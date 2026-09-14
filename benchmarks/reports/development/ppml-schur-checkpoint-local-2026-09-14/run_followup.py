import os
from pathlib import Path
import os,json,hashlib,copy
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
from compare_routes import compare_routes
r=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());requests=json.loads((r/'followup-requests.json').read_text())
assert all(sha(r/'baseline'/p)==h for p,h in ids['baseline_inputs'].items())
assert all(sha(r/'candidate'/p)==h and sha(repo/p)==h for p,h in ids['candidate_inputs'].items())
assert sha(repo/'dist/fereg_macos.plugin')==ids['installed_dist']
for arm in ['baseline','candidate']:assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
oracle=json.loads((r/'oracle-results.json').read_text());assert oracle['process_rc']==0 and oracle['terminal_rc']==['0'] and oracle['sentinel'] and len(oracle['fits'])==9
harness=json.loads((r/'harness-identities.json').read_text());assert all(sha(repo/p)==h for p,h in harness['repository_files'].items())
rows=[]
for i,req in enumerate(requests):
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 row={'name':req['name']};paths={};order=['baseline','candidate'] if i%2==0 else ['candidate','baseline']
 for arm in order:
  out=r/'followup-runs'/(req['name']+'-'+arm);manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=r/arm,solver='auto',identity={'artifact':ids[arm+'_plugin'],'baseline_source':ids['baseline_commit'],'purpose':'Bounded Schur checkpoint screen, unchanged ordinary auto route','protocol':sha(r/'followup-protocol.md'),'requests':sha(r/'followup-requests.json')})
  row[arm]=result;paths[arm]=out;(r/'followup-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(req['name'],arm,result['status'],result['seconds'],flush=True)
  if result['status']=='pass':row[arm+'_runtime']=named_runtime(out)
 if all(row[a]['status']=='pass' for a in order):row['science']=compare_routes(paths['baseline'],paths['candidate'])
 else:row['science']='incomplete: preserve every failure or timeout, no passing speed claim'
 rows.append(row);(r/'followup-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('CHECKPOINT_SCHUR_FOLLOWUP_TERMINAL_COMPLETE',flush=True)
