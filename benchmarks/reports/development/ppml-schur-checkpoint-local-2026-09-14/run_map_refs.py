import os
from pathlib import Path
import copy,json,hashlib
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
from compare_routes import compare_routes
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());assert all(sha(r/'baseline'/p)==h for p,h in ids['baseline_inputs'].items());assert sha(r/'baseline/dist/fereg_macos.plugin')==ids['baseline_plugin']
requests=[q for file in ['requests.json','followup-requests.json'] for q in json.loads((r/file).read_text()) if q['cell']['name']=='strong_fw'];assert len(requests)==3
rows=[]
for req in requests:
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 out=r/'map-runs'/req['name'];manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
 result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=r/'baseline',solver='map',identity={'artifact':ids['baseline_plugin'],'protocol':sha(r/'followup-protocol.md'),'purpose':'Best existing explicit route for original strong FW target'})
 row={'name':req['name'],'result':result};rows.append(row);(r/'map-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(req['name'],result['status'],result['seconds'],flush=True)
 if result['status']=='pass':
  candidate=r/('followup-runs' if 'kind' in req else 'screen-runs')/(req['name']+'-candidate')
  row['science']=compare_routes(out,candidate);row['runtime']=named_runtime(out)
  (r/'map-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('CHECKPOINT_MAP_REFERENCES_TERMINAL_COMPLETE',flush=True)
