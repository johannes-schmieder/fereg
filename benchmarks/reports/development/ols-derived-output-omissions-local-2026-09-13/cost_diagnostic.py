import os
from pathlib import Path
import copy,json,hashlib
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
ROOT=Path(os.environ['OMISSION_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((ROOT/'identities.json').read_text())
req=json.loads((ROOT/'cost-requests.json').read_text())['pw_partial']
rows=[]
for name,sequence in [('aa1',[('first','baseline'),('second','baseline')]),
                      ('ab1',[('candidate','candidate'),('baseline','baseline')]),
                      ('aa2',[('first','baseline'),('second','baseline')]),
                      ('ab2',[('baseline','baseline'),('candidate','candidate')])]:
    paths={};results={}
    for label,arm in sequence:
        out=ROOT/'cost-diagnostic'/f'{name}-{label}'
        settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
        result=measure(ROOT,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,settings,
                       source=ROOT/arm,identity={'artifact':ids[arm+'_plugin'],'kind':name,
                                               'protocol':sha(ROOT/'cost-diagnostic-protocol.md')})
        print(json.dumps({'name':name,'label':label,'status':result['status'],'seconds':result['seconds']}),flush=True)
        assert result['status']=='pass'
        certify(out);paths[label]=out;results[label]=result
    before,after=('first','second') if name.startswith('aa') else ('baseline','candidate')
    baseline_compare(paths[after],paths[before])
    for f in ['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset','saved_outputs']:
        a,b=paths[before]/(f+'.csv'),paths[after]/(f+'.csv');assert a.exists()==b.exists()
        if a.exists():assert a.read_bytes()==b.read_bytes(),(name,f)
    cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15]
    assert np.array_equal(matrix(paths[before],'fereg_solver')[:,cols],matrix(paths[after],'fereg_solver')[:,cols])
    a,b=results[before],results[after];delta=b['seconds']-a['seconds']
    row=dict(name=name,before=a['seconds'],after=b['seconds'],delta=delta,percent=100*delta/a['seconds'],
             review=delta>max(.01,.05*a['seconds']),exact_science=True)
    rows.append(row);(ROOT/'cost-diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(row),flush=True)
