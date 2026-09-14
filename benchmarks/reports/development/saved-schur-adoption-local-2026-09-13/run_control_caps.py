import os
from pathlib import Path
import json,copy,hashlib,os
from cap_measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare
import pandas as pd,numpy as np
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text())
assert all(sha(root/'candidate'/p)==h for p,h in ids['candidate_inputs'].items());assert sha(root/'candidate/dist/fereg_macos.plugin')==ids['candidate_plugin']
requests=json.loads((root/'cost-requests.json').read_text());bounds=json.loads((root/'control-admission-replay.json').read_text())['rows'];rows=[]
for name in ['disconnected_blocks','unbalanced','auto_cmg_saved','weak_d3']:
 req=next(x for x in requests if x['name']==name);bound=next(x['initial_schur_cg_joint'] for x in bounds if x['name']==name)
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 os.environ['FEREG_MEMORY_LIMIT_BYTES']=str(bound+536870912);out=root/'control-cap-runs'/name
 result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,copy.deepcopy(MANIFEST),source=root/'candidate',trace=True,identity={'artifact':ids['candidate_plugin'],'usable_cap':bound,'purpose':'constrained protected graph diagnostic; excluded from timing aggregates'})
 row={'name':name,'usable_cap':bound,'status':result['status'],'metadata':result['metadata']};rows.append(row);(root/'control-cap-results.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert result['status']=='pass',row
 row['native_science']=certify(out);ref=root/'cost-runs'/(name+'-candidate');row['comparison']=baseline_compare(out,ref)
 for f in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:assert (out/f).read_bytes()==(ref/f).read_bytes(),(name,f)
 a=pd.read_csv(out/'saved_outputs.csv').set_index('obs_id');b=pd.read_csv(ref/'saved_outputs.csv').set_index('obs_id');assert a.index.equals(b.index) and a.columns.equals(b.columns)
 row['saved_differences']={}
 for col in a:
  av=pd.to_numeric(a[col],errors='coerce');bv=pd.to_numeric(b[col],errors='coerce');assert av.isna().equals(bv.isna());delta=np.abs(av.dropna().to_numpy()-bv.dropna().to_numpy());limit=1e-9+1e-7*np.abs(bv.dropna().to_numpy());assert np.all(delta<=limit),(name,col,float(delta.max()));row['saved_differences'][col]=float(delta.max())
 assert result['metadata']['fereg_native_allocation_bytes']<=bound
 row['admitted_peak']=True;row['trace']=[z for z in (out/'stderr.txt').read_text().splitlines() if z.startswith('FEREG_PHASE')]
 (root/'control-cap-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,'PASS','native',result['metadata']['fereg_native_allocation_bytes'],'usable',bound,flush=True)
print('CONTROL_CAPS_TERMINAL_PASS',flush=True)
