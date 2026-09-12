"""Collect numeric local evidence, excluding datasets and licensed logs."""
import argparse, hashlib, json, re, statistics, zipfile
from pathlib import Path
import numpy as np
import pandas as pd
from common import read_json, write_json


def mat(path,name):
 return pd.read_csv(path/(name+'.csv')).pivot(index='row',columns='column',values='value')

def verify(a,b):
 ra,rb=read_json(a/'result.json'),read_json(b/'result.json')
 assert ra['sample_sha256']==rb['sample_sha256'] and ra['metadata']['N']==rb['metadata']['N']
 assert ra['input_sha256']==rb['input_sha256']
 out={'sample_exact':True,'reference_run':b.name}
 for file,keys,cols in [('coefficients.csv',['name'],['value','se']),('covariance.csv',['row','column'],['value'])]:
  x=pd.read_csv(a/file).set_index(keys).sort_index();y=pd.read_csv(b/file).set_index(keys).sort_index()
  assert x.index.equals(y.index)
  for col in cols:
   rtol=1e-6 if file=='covariance.csv' or col=='se' else 1e-7
   assert np.isfinite(x[col]).all() and np.isfinite(y[col]).all()
   assert np.allclose(x[col],y[col],atol=1e-10,rtol=rtol)
   out[file+':'+col]=float(np.max(np.abs(x[col]-y[col])))
 assert np.isclose(ra['metadata']['rss'],rb['metadata']['rss'],rtol=1e-8,atol=1e-10)
 assert ra['metadata']['N']-ra['metadata']['df_r']==rb['metadata']['df_k']
 out['residual_degrees_of_freedom_exact']=True
 c=mat(a,'fereg_certificate')
 assert all(c.loc[row,1]==c.loc[row,2]==1 for row in [1,2,3,24,32])
 s=mat(a,'fereg_solver');assert np.all(s[4]==2) and np.all(s[6]<=ra['metadata']['fereg_tolerance'])
 out['native_certificates_pass']=True
 return out

p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('output',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
paths=sorted(p.parent for p in (a.root/'runs').glob('*/result.json'))
results={p:read_json(p/'result.json') for p in paths}
rows=[];phase_records={};checks={};unmatched=[]
for path,r in results.items():
 req=read_json(path/'request.json');m=r['metadata'];cell=r['cell']
 row={'run':path.name,'arm':r['arm'],'design':cell['design'],'n':cell['n'],'k':cell['k'],'threads':cell['threads'],
      'trace':bool(req.get('trace',False)),'status':r['status'],'seconds':r['seconds'],'peak_rss_gib':r['peak_rss_gib'],
      'input_sha256':r['input_sha256'],'seed':(r.get('measurement_fingerprint') or {}).get('seed'),
      'poolsize':m.get('fereg_poolsize'),'load_average_1m':r['system_load_average_at_launch'][0]}
 if r['arm']=='fereg' and r['status']=='pass':
  rt=mat(path,'fereg_runtime');row.update({'input_load_seconds':rt.loc[19,2],'native_seconds':rt.loc[20,2],
      'outer_seconds':r['seconds']-rt.loc[21,2],'preflight_seconds':rt.loc[18,2],
      'native_reported_allocation_bytes':m.get('fereg_native_allocation_bytes'),'modeled_fast_bytes':m.get('fereg_memory_fast_bytes')})
  matches=[q for q,s in results.items() if s['arm']=='fixest' and s['status']=='pass' and s['input_sha256']==r['input_sha256'] and s['cell']['threads']==cell['threads']]
  if matches:checks[path.name]=verify(path,matches[-1])
  else:unmatched.append(path.name)
 rows.append(row)
 lines=[]
 for f in [path/'stderr.txt',*path.glob('*.log')]:
  if f.exists():lines.extend(x for x in f.read_text(errors='replace').splitlines() if x.startswith('FEREG_PHASE|'))
 if lines:phase_records[path.name]=list(dict.fromkeys(lines))
df=pd.DataFrame(rows);df.to_csv(a.output/'measurements.csv',index=False)
for prefix,file in [('repeat-','primary-summary.csv'),('confirm-','vector-summary.csv')]:
 d=df[df.run.str.startswith(prefix)].copy();d['variant']=d.run.str.split('-').str[1]
 out=d.groupby(['design','threads','variant']).agg(runs=('seconds','size'),median_seconds=('seconds','median'),min_seconds=('seconds','min'),max_seconds=('seconds','max'),median_peak_rss_gib=('peak_rss_gib','median')).reset_index()
 out.to_csv(a.output/file,index=False)
one=df[df.run.str.startswith('repeat-baseline-one_fe')].groupby('threads')[['seconds','input_load_seconds','native_seconds','outer_seconds','preflight_seconds','peak_rss_gib']].median()
one.to_csv(a.output/'one-fe-phases.csv')
write_json(a.output/'phase-timings.json',phase_records)
write_json(a.output/'science.json',{'schema':'fixest-gap-local-science-v1','status':'pass','checked_fereg_runs':len(checks),
 'fixest_comparisons':checks,'no_matching_fixest_run':unmatched,
 'independent_oracles':{p.name:read_json(p/'oracle-comparison.json') for p in paths if (p/'oracle-comparison.json').exists()},
 'full_native_comparisons':{p.name:read_json(p/'baseline-comparison.json') for p in paths if (p/'baseline-comparison.json').exists()}})
allowed=['result.json','import-hashes.json','coefficients.csv','covariance.csv','b.csv','V.csv','fereg_solver.csv','fereg_runtime.csv','fereg_certificate.csv','fereg_fit.csv','fereg_status.csv','fereg_dof.csv','native-science.json','baseline-comparison.json','independent-oracle.json','oracle-comparison.json']
with zipfile.ZipFile(a.output/'evidence.zip','w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 for path in paths:
  for name in allowed:
   f=path/name
   if f.exists():
    info=zipfile.ZipInfo(path.name+'/'+name,date_time=(2026,9,11,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,f.read_bytes())
 for f in sorted((a.root/'data').glob('*.json')):
  info=zipfile.ZipInfo('input-identities/'+f.name,date_time=(2026,9,11,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,f.read_bytes())
print(json.dumps({'runs':len(rows),'all_worker_statuses':df.status.value_counts().to_dict(),'fixest_checks':len(checks),'independent_oracles':len(list((a.root/'runs').glob('*/oracle-comparison.json')))},indent=2))
