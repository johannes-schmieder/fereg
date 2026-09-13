import os
from pathlib import Path
import os, json, hashlib, copy, re
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST, compare
from final_compare import matrix, certify, baseline_compare
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'v3-built-identities.json').read_text())
source={'baseline':root/'baseline','candidate':root/'candidate-v3'}
for arm in source:
 assert sha(source[arm]/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
 assert all(sha(source[arm]/p)==h for p,h in ids[arm+'_inputs'].items())
tests=json.loads((root/'v3-test-results.json').read_text());assert len(tests)==5 and all(x['rc']==0 for x in tests)
assert '95 passed; 0 failed' in (root/'v3-plugin-serial.log').read_text()
validation=json.loads((root/'v3-validation-results.json').read_text());assert len(validation)==4 and all(x['sentinel'] for x in validation)
cap=(root/'validation/generic-v3-exact/run.log').read_text()
peak=int(re.search(r'^GENERIC_CAP_PEAK=\s*(\d+)',cap,re.M).group(1));assert peak<=806328
previous=Path(os.environ['SAVED_PRECISION_ROOT'])
controls=json.loads((previous/'control-requests.json').read_text());primary=json.loads((previous/'primary-requests.json').read_text())
slope=next(x for x in controls if x['name']=='slope')
requests=[slope]
for solver in ['lsmr','cg']:
 req=copy.deepcopy(slope);req['name']='slope_'+solver;req['solver']=solver;requests.append(req)
requests += [next(x for x in primary if x['name']==name and x['seed']==104729) for name in ['unit_saved','pw_partial']]
requests += [next(x for x in controls if x['name']==name) for name in ['complete_iid','weak_d3','weighted_cut','small_aw']]
p=Path(os.path.join(os.environ['CG_ENERGY_ROOT'], 'runs/control-ppml_weak_pw-s31415927-baseline/request.json'));req=json.loads(p.read_text());requests.append({'name':'ppml_weak_pw','cell':req['cell'],'data':req['data'],'warm':req['warm'],'data_sha256':sha(Path(req['data'])),'warm_sha256':sha(Path(req['warm']))})
assert len(requests)==10
(root/'cost-requests.json').write_text(json.dumps(requests,indent=2)+'\n')
results=[]
def pair(req,index,suffix=''):
 name=req['name'];paths={};data={};row={'name':name+suffix,'source_name':name,'input_sha256':req['data_sha256'],'solver':req.get('solver','auto')};ols=req['cell']['model']=='ols'
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for arm in (['baseline','candidate'] if index%2==0 else ['candidate','baseline']):
  out=root/'cost-runs'/f'{name}{suffix}-{arm}';spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=180
  assert not out.exists()
  value=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,spec,source=source[arm],solver=req.get('solver','auto'),identity={'artifact':ids[arm+'_plugin'],'purpose':'generic owned FE admission repair cost','protocol':sha(root/'admission-protocol.md'),'harness':sha(Path(__file__))})
  print(name+suffix,arm,value['status'],value['seconds'],flush=True);assert value['status']=='pass',out
  assert 'solver('+req.get('solver','auto')+')' in value['metadata']['command']
  if ols:certify(out)
  paths[arm]=out;data[arm]=value;row[arm]={'seconds':value['seconds'],'metadata':value['metadata'],'rss_gib':value.get('peak_rss_gib')}
 row['fit_comparison']=baseline_compare(paths['candidate'],paths['baseline']) if ols else compare(paths['candidate'],paths['baseline'],'baseline')
 compared=[]
 for f in paths['baseline'].glob('*.csv'):
  if any(s in f.name for s in ['runtime','solver']):continue
  other=paths['candidate']/f.name;assert other.read_bytes()==f.read_bytes(),(name,f.name)
  compared.append(f.name)
 row['exact_scientific_csvs']=sorted(compared)
 if ols:
  a,b=[matrix(paths[arm],'fereg_solver') for arm in ['baseline','candidate']]
  assert a.shape==b.shape and np.array_equal(np.delete(a,[8,9],axis=1),np.delete(b,[8,9],axis=1)),name
  row['exact_non_time_solver_fields']=True
 delta=data['candidate']['seconds']-data['baseline']['seconds'];row.update(delta=delta,percent=100*delta/data['baseline']['seconds'],review=delta>max(.01,.05*data['baseline']['seconds']))
 results.append(row);(root/'cost-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print('PAIR',name+suffix,'change',delta,'review',row['review'],flush=True);return row
flagged=[]
for i,req in enumerate(requests):
 row=pair(req,i)
 if row['review']:flagged.append((req,i))
for req,i in flagged:
 for repeat in [1,2]:pair(req,i+1,f'-recheck{repeat}')
assert all(sha(source[arm]/'dist/fereg_macos.plugin')==ids[arm+'_plugin'] for arm in source)
print('COST_COMPARISONS_TERMINAL_PASS',flush=True)
