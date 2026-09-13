import os
from pathlib import Path
import os,json,hashlib,copy
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST,compare
from final_compare import matrix,certify,baseline_compare
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text())
for arm in ['baseline','candidate']:
 assert sha(root/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
 assert all(sha(root/arm/p)==h for p,h in ids[arm+'_inputs'].items())
assert all(x['rc']==0 for x in json.loads((root/'test-results.json').read_text()))
assert len(json.loads((root/'original-replay-results.json').read_text()))==2
assert 'ORIGINAL_REPLAY_TERMINAL_PASS' in (root/'original-replay.log').read_text()
assert 'TERMINAL_CMG_ORACLE_PASS' in (root/'terminal-cmg.log').read_text()
assert '1 passed; 0 failed' in (root/'terminal-budget-test.log').read_text()
validation=json.loads((root/'validation-results.json').read_text());assert len(validation)==4 and all(x['sentinel'] for x in validation);assert len(json.loads((root/'terminal-cmg-results.json').read_text()))==2
controls=json.loads((root/'control-requests.json').read_text());primary=json.loads((root/'primary-requests.json').read_text())
requests=[next(x for x in primary if x['name']==n and x['seed']==104729) for n in ['unit_saved','pw_partial']]
requests += [next(x for x in controls if x['name']==n) for n in ['rank_deficient_saved','slope','three_fe_saved','explicit_cg_saved','weak_d3','low_fw','complete_iid','weighted_cut','small_aw']]
p=Path(os.path.join(os.environ['CG_ENERGY_ROOT'], 'runs/control-ppml_weak_pw-s31415927-baseline/request.json'));req=json.loads(p.read_text());requests.append({'name':'ppml_weak_pw','cell':req['cell'],'data':req['data'],'warm':req['warm'],'data_sha256':sha(Path(req['data'])),'warm_sha256':sha(Path(req['warm']))})
extra=copy.deepcopy(next(x for x in controls if x['name']=='explicit_cg_saved'));extra.update(name='auto_cmg_saved',solver='auto');requests.append(extra)
(root/'cost-requests.json').write_text(json.dumps(requests,indent=2)+'\n')
oracles={'auto_cmg_saved':Path(os.path.join(os.environ['FEREG_SAVED_RESIDUAL_REPAIR_20260913_ROOT'], 'auto-cmg-direct-oracle.csv')),'explicit_cg_saved':Path(os.path.join(os.environ['FEREG_SAVED_RESIDUAL_REPAIR_20260913_ROOT'], 'auto-cmg-direct-oracle.csv')),'three_fe_saved':Path(os.path.join(os.environ['FEREG_SAVED_RESIDUAL_REPAIR_20260913_ROOT'], 'three-fe-direct-oracle.csv')),'pw_partial':Path(os.path.join(os.environ['FEREG_SAVED_SCHUR_POLISH_20260913_ROOT'], 'pw-s104729-direct-oracle.csv')),'rank_deficient_saved':Path(os.path.join(os.environ['FEREG_SAVED_SCHUR_FINAL_20260913_ROOT'], 'rank-direct-oracle.csv')),'weak_d3':Path(os.path.join(os.environ['SAVED_SCREEN_ROOT'], 'weak-direct-oracle.csv'))}
results=[]
def outputs(path):
 f=path/'saved_outputs.csv'
 if not f.exists():return None
 d=pd.read_csv(f);return d.set_index('obs_id').apply(pd.to_numeric,errors='coerce')
def pair(req,index,suffix=''):
 name=req['name'];paths={};data={};row={'name':name+suffix,'source_name':name,'input_sha256':req['data_sha256']};ols=req['cell']['model']=='ols'
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for arm in (['baseline','candidate'] if index%2==0 else ['candidate','baseline']):
  out=root/'cost-runs'/f'{name}{suffix}-{arm}';spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=180
  if (out/'result.json').exists():
   value=json.loads((out/'result.json').read_text());assert value['measurement_fingerprint']['artifact']==ids[arm+'_plugin'];saved_req=json.loads((out/'request.json').read_text());assert saved_req['cell']==req['cell'] and saved_req['data']==req['data'] and saved_req['warm']==req['warm']
  else:
   value=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,spec,source=root/arm,solver=req.get('solver','auto'),identity={'artifact':ids[arm+'_plugin'],'purpose':'independent saved final precision correctness cost','protocol':sha(root/'cost-protocol.md'),'harness':sha(Path(__file__))})
  print(name+suffix,arm,value['status'],value['seconds'],flush=True);assert value['status']=='pass',out;assert "solver("+req.get('solver','auto')+")" in value['metadata']['command']
  if ols:certify(out)
  paths[arm]=out;data[arm]=value;row[arm]={'seconds':value['seconds'],'metadata':value['metadata'],'rss_gib':value.get('peak_rss_gib')}
 if ols:
  row['fit_comparison']=baseline_compare(paths['candidate'],paths['baseline'])
  for f in ['fereg_status','fereg_dof','fereg_fe','fereg_cluster','fereg_subset']:assert (paths['baseline']/(f+'.csv')).read_bytes()==(paths['candidate']/(f+'.csv')).read_bytes(),(name,f)
 else:
  row['fit_comparison']=compare(paths['candidate'],paths['baseline'],'baseline')
  for f in paths['baseline'].glob('*.csv'):
   if any(s in f.name for s in ['runtime','solver']):continue
   other=paths['candidate']/f.name;assert other.read_bytes()==f.read_bytes(),(name,f.name)
 a,b=(outputs(paths[arm]) for arm in ['baseline','candidate']);row['saved_parity']=True;row['saved_differences']={}
 if a is not None:
  assert a.index.equals(b.index) and a.columns.equals(b.columns)
  for col in a:
   assert np.array_equal(a[col].isna(),b[col].isna());av,bv=a[col].dropna().to_numpy(),b[col].dropna().to_numpy();error=np.abs(av-bv);limit=1e-9+1e-7*np.abs(av);bad=int(np.count_nonzero(error>limit));row['saved_differences'][col]={'maxabs':float(error.max()),'failing_rows':bad};row['saved_parity'] &= bad==0
 if name in oracles:
  truth=pd.read_csv(oracles[name]).set_index('obs_id');row['independent_outputs']={};row['oracle_sha256']=sha(oracles[name])
  for arm,d in [('baseline',a),('candidate',b)]:
   row['independent_outputs'][arm]={}
   for col in d:
    if col not in truth:continue
    s=d[col].dropna();assert s.index.equals(truth.index);actual=s.to_numpy()
    if name=='three_fe_saved' and col=='saved_g':
     component=truth.component.to_numpy(dtype=int);actual=actual-(np.bincount(component,weights=actual)/np.bincount(component))[component]
    e=np.abs(actual-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy());bad=int(np.count_nonzero(e>limit));row['independent_outputs'][arm][col]={'failing_rows':bad,'maxabs':float(e.max()),'max_tolerance_ratio':float(np.max(e/limit))}
    if arm=='candidate':assert bad==0,(name,col,bad)
 elif not row['saved_parity']:raise AssertionError((name,row['saved_differences']))
 row['accuracy_matched_timing']=row['saved_parity'] and all(x['failing_rows']==0 for x in row.get('independent_outputs',{}).get('baseline',{}).values())
 delta=data['candidate']['seconds']-data['baseline']['seconds'];row.update(delta=delta,percent=100*delta/data['baseline']['seconds'],review=delta>max(.01,.05*data['baseline']['seconds']))
 results.append(row);(root/'cost-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if k not in ['baseline','candidate','fit_comparison','independent_outputs','saved_differences']}),flush=True);return row
flagged=[]
for i,req in enumerate(requests):
 row=pair(req,i)
 if row['review']:flagged.append((req,i))
for req,i in flagged:
 for repeat in [1,2]:pair(req,i+1,f'-recheck{repeat}')
print('COST_COMPARISONS_TERMINAL_PASS',flush=True)
