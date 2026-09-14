import os
from pathlib import Path
import os,json,hashlib,copy,re,sys
import pandas as pd,numpy as np
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text())
for arm,key in [('baseline','inputs'),('candidate','candidate_inputs')]:
 assert sha(root/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin'];assert all(sha(root/arm/p)==h for p,h in ids[key].items())
checks=json.loads((root/'test-results.json').read_text());assert len(checks)==7 and all(x['rc']==0 for x in checks)
validation=json.loads((root/'validation-results.json').read_text());assert all(x['sentinel'] and x['terminal_rc']==['0'] for x in validation[:3])
for arm in ['baseline','candidate']:
 p=root/'validation'/('generated-'+arm)/'run.log';assert re.search(r'^END_FIXTURE\|low_score_generated_supported\|fits=1\|rc=0\s*$',p.read_text(errors='replace'),re.M)
for name,marker in [('layouts','LOW_SCORE_VALIDATION_RC'),('unsupported-baseline','LAYOUT_SUPPLEMENT_RC')]:
 txt=(root/'validation'/name/'run.log').read_text(errors='replace');assert re.search(r'^'+marker+r'=498\s*$',txt,re.M);assert 'unsupported: prepare-ols-test supports only' in txt
rows=[];requests=json.loads((root/'followup-requests.json').read_text());part=sys.argv[1] if len(sys.argv)>1 else 'screen';selected=[x for x in requests if x['role']==('primary' if part=='screen' else 'control')]
for index,req in enumerate(selected):
 paths={};results={};name=req['name'];os.environ['FEREG_LOCAL_POOL']=str(req['cell'].get('pool',10));os.environ['FEREG_LOCAL_BLOCK_WIDTH']=req.get('block_width','1')
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for arm in (['baseline','candidate'] if index%2==0 else ['candidate','baseline']):
  out=root/'runs'/(part+'-'+name+'-'+arm);settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
  value=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,settings,source=root/arm,solver=req.get('solver','auto'),identity={'artifact':ids[arm+'_plugin'],'baseline_commit':ids['baseline_commit'],'protocol':sha(root/'protocol.md'),'requests':sha(root/'followup-requests.json'),'harness':sha(Path(__file__))})
  print(name,arm,value['status'],value['seconds'],flush=True);assert value['status']=='pass',out;certify(out);assert value['metadata']['fereg_poolsize']==req['cell'].get('pool',10)
  paths[arm]=out;results[arm]=value
 comparison=baseline_compare(paths['candidate'],paths['baseline'])
 for name_csv in ['fereg_status','fereg_dof','fereg_fe','fereg_cluster','fereg_subset']:
  assert (paths['candidate']/(name_csv+'.csv')).read_bytes()==(paths['baseline']/(name_csv+'.csv')).read_bytes(),(name,name_csv)
 # Initial/final fit solvers are unchanged; removed inference projection lives
 # outside these two rows. Preserve rank, iteration, warning/fallback fields.
 cols=[0,1,2,3,10,11,12,13,14,15]
 assert np.array_equal(matrix(paths['candidate'],'fereg_solver')[:,cols],matrix(paths['baseline'],'fereg_solver')[:,cols]),name
 a,b=paths['candidate']/'saved_outputs.csv',paths['baseline']/'saved_outputs.csv'
 if a.exists():
  av,bv=pd.read_csv(a),pd.read_csv(b);assert list(av)==list(bv);assert np.allclose(av.to_numpy(float),bv.to_numpy(float),atol=1e-9,rtol=1e-7,equal_nan=True)
 baseline,candidate=results['baseline']['seconds'],results['candidate']['seconds'];delta=candidate-baseline
 row={'name':name,'baseline':results['baseline'],'candidate':results['candidate'],'comparison':comparison,'solver_decisions_exact':True,'delta':delta,'percent':100*delta/baseline,'qualifying_time_gain':-delta>max(.01,.05*baseline),'cost_flag':delta>max(.01,.05*baseline)};rows.append(row);(root/(part+'-results.json')).write_text(json.dumps(rows,indent=2)+'\n');print(name,'paired change',row['percent'],'gain',row['qualifying_time_gain'],'costflag',row['cost_flag'],flush=True)
print('LOW_SCORE_'+part.upper()+'_TERMINAL_PASS',flush=True)
