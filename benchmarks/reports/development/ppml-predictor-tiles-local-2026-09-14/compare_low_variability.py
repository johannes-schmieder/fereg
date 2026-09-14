import os
from pathlib import Path
import os,json,hashlib,copy
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import compare,named_runtime
r=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((r/'built-identities.json').read_text())
req=next(x for x in json.loads((r/'followup-requests.json').read_text()) if x['name']=='low_fw')
for arm,key in [('baseline','inputs'),('candidate','candidate_inputs')]:
    assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
    assert all(sha(r/arm/p)==h for p,h in ids[key].items())
assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
rows=[]
for repeat in range(6):
    for kind in (['AA','AB'] if repeat%2==0 else ['AB','AA']):
        row={'name':'low_fw','kind':kind,'repeat':repeat};paths={}
        os.environ['FEREG_LOCAL_POOL']=str(req['cell'].get('pool',10))
        os.environ['FEREG_LOCAL_BLOCK_WIDTH']=req.get('block_width','1')
        for arm in (['baseline','candidate'] if repeat%2==0 else ['candidate','baseline']):
            source_arm='baseline' if kind=='AA' else arm
            out=r/'variability-runs'/(kind+'-'+str(repeat)+'-'+arm)
            settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
            result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,settings,source=r/source_arm,solver=req.get('solver','auto'),identity={'artifact':ids[source_arm+'_plugin'],'baseline_commit':ids['baseline_commit'],'protocol':sha(r/'low-variability-protocol.md'),'harness':sha(Path(__file__))})
            row[arm]=result;paths[arm]=out
            (r/'low-variability-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n')
            print(kind,repeat,arm,result['status'],result['seconds'],flush=True)
            assert result['status']=='pass',out
        compare(paths)
        row['science']=json.loads((paths['candidate']/'exact-science.json').read_text())
        row['runtime']={a:named_runtime(p) for a,p in paths.items()}
        b,c=row['baseline']['seconds'],row['candidate']['seconds']
        row.update(delta=c-b,percent=100*(c-b)/b,cost_flag=c-b>max(.01,.05*b))
        rows.append(row)
        (r/'low-variability-results.json').write_text(json.dumps(rows,indent=2)+'\n')
        print('pair',kind,repeat,row['delta'],row['percent'],'costflag',row['cost_flag'],flush=True)
print('PREDICTOR_LOW_VARIABILITY_TERMINAL_PASS',flush=True)
