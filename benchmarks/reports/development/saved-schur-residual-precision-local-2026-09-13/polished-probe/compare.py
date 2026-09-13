import os
from pathlib import Path
import argparse,json,copy,hashlib
import numpy as np
import pandas as pd
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
ROOT=Path(os.environ['FEREG_SAVED_SCHUR_POLISH_20260913_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def compare_pair(request,rotation,kind='primary',suffix=''):
 ids=json.loads((ROOT/'built-identities.json').read_text());assert sha(ROOT/'protocol.md')==ids['protocol']
 cell=request['cell'];name=request['name'];seed=request['seed'];paths={}
 for arm in (['baseline','candidate'] if rotation%2==0 else ['candidate','baseline']):
  source=ROOT/arm;assert sha(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
  data,warm=Path(request['data']),Path(request['warm']);assert sha(data)==request['data_sha256'] and sha(warm)==request['warm_sha256']
  output=ROOT/'runs'/f'{kind}-{name}-s{seed}{suffix}-{arm}';paths[arm]=output
  manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
  result=measure(ROOT,cell,'fereg','warm',data,warm,output,manifest,source=source,solver=request.get('solver','auto'),identity={'baseline_commit':ids['baseline_commit'],'plugin':ids[arm+'_plugin'],'protocol':ids['protocol'],'harness':sha(Path(__file__)),'kind':kind,'seed':seed})
  print(json.dumps({'kind':kind,'name':name,'seed':seed,'arm':arm,'status':result['status'],'seconds':result['seconds'],'native_peak':result['metadata'].get('fereg_native_allocation_bytes')}),flush=True)
  assert result['status']=='pass',str(output)
  (output/'native-science.json').write_text(json.dumps(certify(output),indent=2)+'\n')
 comparison=baseline_compare(paths['candidate'],paths['baseline'])
 for filename in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:
  a,b=(paths[arm]/filename for arm in ['baseline','candidate']);assert a.exists()==b.exists()
  if a.exists():assert a.read_bytes()==b.read_bytes(),(name,filename)
 saved={}
 parity_failures=[];oracle_review={}
 a,b=(paths[arm]/'saved_outputs.csv' for arm in ['baseline','candidate']);assert a.exists()==b.exists()
 if a.exists():
  a,b=pd.read_csv(a),pd.read_csv(b);assert a.shape==b.shape and list(a.columns)==list(b.columns)
  assert np.array_equal(a.obs_id,b.obs_id)
  for col in a.columns[1:]:
   x,y=(pd.to_numeric(frame[col],errors='coerce').to_numpy() for frame in [a,b]);assert np.array_equal(np.isnan(x),np.isnan(y))
   if not np.allclose(x,y,atol=1e-9,rtol=1e-7,equal_nan=True):parity_failures.append(col)
   saved[col]=float(np.nanmax(np.abs(x-y)))
 if parity_failures:
  original=Path(os.environ['SAVED_SCREEN_ROOT'])
  assert name in ['weak_d3','disconnected_blocks','unbalanced'],(name,parity_failures)
  old_request=next(r for r in json.loads((original/'control-requests.json').read_text()) if r['name']==name)
  assert request['data_sha256']==old_request['data_sha256'] and request['seed']==old_request['seed']
  oracle_file=original/f'{name}-direct-oracle.csv'
  if name=='weak_d3':oracle_file=original/'weak-direct-oracle.csv'
  truth=pd.read_csv(oracle_file);oracle_review['oracle_sha256']=sha(oracle_file)
  for arm,frame in [('baseline',a),('candidate',b)]:
   assert np.array_equal(frame.obs_id,truth.obs_id)
   checks={}
   for col in frame.columns[1:]:
    error=np.abs(frame[col].to_numpy()-truth[col].to_numpy());limit=1e-9+1e-7*np.abs(truth[col].to_numpy())
    checks[col]={'maxabs':float(error.max()),'failing_rows':int(np.count_nonzero(error>limit)),'max_tolerance_ratio':float(np.max(error/limit))}
   oracle_review[arm]=checks
  assert all(v['failing_rows']==0 for v in oracle_review['candidate'].values()),(name,oracle_review)
  assert all(oracle_review['baseline'][col]['failing_rows']>0 for col in parity_failures),(name,oracle_review)
 solver=matrix(paths['candidate'],'fereg_solver');baseline_solver=matrix(paths['baseline'],'fereg_solver')
 if kind=='primary':assert solver[0,14]==8,(name,solver)
 changed_probe=solver[0,14]!=baseline_solver[0,14] or solver[0,15]!=baseline_solver[0,15]
 accepted=changed_probe and solver[0,14]==8 and solver[0,15]<=.01
 if not accepted:
  for file in ['b.csv','V.csv','fereg_fit.csv','fereg_certificate.csv','saved_outputs.csv']:
   a,b=paths['baseline']/file,paths['candidate']/file;assert a.exists()==b.exists()
   if a.exists():assert a.read_bytes()==b.read_bytes(),(name,seed,file)
 br,cr=(json.loads((paths[arm]/'result.json').read_text()) for arm in ['baseline','candidate']);delta=cr['seconds']-br['seconds'];mb,mc=(r['metadata']['fereg_native_allocation_bytes'] for r in [br,cr])
 row={'kind':kind,'name':name,'seed':seed,'suffix':suffix,'baseline_seconds':br['seconds'],'candidate_seconds':cr['seconds'],'delta_seconds':delta,'percent':100*delta/br['seconds'],'review':delta>max(.010,.05*br['seconds']),'command_qualifies':-delta>=.010 and -delta/br['seconds']>=.05,'memory_qualifies':mb-mc>=2*1024**2 and (mb-mc)/mb>=.05 and cr['seconds']<=1.05*br['seconds'],'baseline_native_peak':mb,'candidate_native_peak':mc,'baseline_metadata':br['metadata'],'candidate_metadata':cr['metadata'],'baseline_rss_gib':br['peak_rss_gib'],'candidate_rss_gib':cr['peak_rss_gib'],'comparison':comparison,'saved_maxabs':saved,'new_probe':bool(changed_probe),'schur_probe_accepted':bool(accepted),'baseline_solver':baseline_solver.tolist(),'candidate_solver':solver.tolist()}
 row.update(parity_pass=not parity_failures,parity_failed_columns=parity_failures,independent_oracle=oracle_review)
 if parity_failures:row['kind']='adjudicated-control'
 (paths['candidate']/'pair.json').write_text(json.dumps(row,indent=2)+'\n')
 resultfile=ROOT/'comparisons.json';rows=json.loads(resultfile.read_text()) if resultfile.exists() else [];rows.append(row);resultfile.write_text(json.dumps(rows,indent=2)+'\n')
 print(json.dumps({k:v for k,v in row.items() if k not in ['baseline_metadata','candidate_metadata','baseline_solver','candidate_solver','comparison']}),flush=True)
 return row
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--kind',choices=['primary','control'],default='primary');a=parser.parse_args()
 assert all(x['sentinel'] for x in json.loads((ROOT/'validation-results.json').read_text()))
 caps=json.loads((ROOT/'initial-cap-results.json').read_text());assert len(caps)==4 and all(x['rc']==(0 if x['offset']==0 else 909) for x in caps)
 requests=json.loads((ROOT/('primary-requests.json' if a.kind=='primary' else 'control-requests.json')).read_text())
 for i,request in enumerate(requests):compare_pair(request,i,a.kind)
