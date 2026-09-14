import os
from pathlib import Path
import json,copy,hashlib,re,os
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'memory-diagnostic-built.json').read_text());source=root/'memory-diagnostic'
assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
assert all(sha(source/p)==h for p,h in ids['inputs'].items())
requests=json.loads((root/'cost-requests.json').read_text());results=[]
for name in ['disconnected_blocks','unbalanced','auto_cmg_saved','weak_d3']:
 req=next(x for x in requests if x['name']==name);assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 out=root/'memory-review-runs'/name
 result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,copy.deepcopy(MANIFEST),source=source,trace=True,identity={'artifact':ids['candidate_plugin'],'purpose':'isolated allocator diagnostic; excluded from paired performance'})
 assert result['status']=='pass';certify(out)
 ref=root/'cost-runs'/(name+'-candidate');comparison=baseline_compare(out,ref)
 for f in ['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv','saved_outputs.csv']:assert (out/f).read_bytes()==(ref/f).read_bytes(),(name,f)
 lines=[z for z in (out/'stderr.txt').read_text().splitlines() if z.startswith(('OLS_ALLOCATION_','OLS_RECOVERY_','FEREG_PHASE'))]
 row={'name':name,'science_exact':True,'comparison':comparison,'lines':lines,'metadata':result['metadata']};results.append(row)
 (root/'memory-review-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name,'PASS',flush=True)
 for z in lines:print(z,flush=True)
print('MEMORY_REVIEW_TERMINAL_PASS',flush=True)
