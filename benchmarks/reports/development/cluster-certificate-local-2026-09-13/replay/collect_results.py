"""Collect the protocol's rejection branch, keeping public profiles separate."""
from pathlib import Path
import csv,json,statistics,sys,zipfile
from common import read_json,write_json,sha256
from collect_preparation import collect as collect_ols
from collect_ppml_preparation import collect as collect_ppml
root,out=map(Path,sys.argv[1:])
note='Local rejected prototype: three single-seed public primary screens, four focused protected controls and two separate public phase profiles. No primary repeats, private IRLS allocation audit, external comparator or qualification. All observations retained.'
for p in sorted((root/'runs').glob('*-candidate')):
 checks=read_json(p/'exact-science.json');assert all(checks.values())
 for q in [p,p.with_name(p.name.removesuffix('-candidate')+'-baseline')]:
  r=read_json(q/'result.json');assert r['status']=='pass' and r['sentinel'] and r['process_returncode']==0 and r['metadata']['rc']==0
collect_ols(root,out/'ols',pair_prefix='prep-clcert',primary_cases=['ols_dense','ols_sparse'],note=note)
collect_ppml(root,out/'ppml',prefix='ppmlprep-clcert',primary_cases={'ppml_sparse'})
# Shared PPML collector uses '-trace-' for an older private profile. This
# driver uses public '-profile-' calls; correct grouping before final review.
data=read_json(out/'ppml/comparisons.json');groups={}
for p in data['pairs']:
 p['group']='profile' if '-profile-' in p['pair'] else 'primary' if p['case']=='ppml_sparse' else 'control'
 for arm in ['baseline','candidate']:
  rt=read_json(root/'runs'/(p['pair']+'-candidate')/'paired-runtime.json')[arm]
  p[arm]['native_peak_bytes']=rt['allocation_peak_delta']
  p[arm]['raw_allocator_peak_bytes']=rt['allocation_peak']
 groups.setdefault((p['group'],p['case']),[]).append(p)
summary=[]
for (group,case),ps in sorted(groups.items()):
 row=dict(group=group,case=case,pairs=len(ps))
 for arm in ['baseline','candidate']:
  for key in ps[0][arm]:
   values=[p[arm][key] for p in ps if p[arm][key] is not None]
   row[arm+'_'+key]=statistics.median(values) if values else None
 row['median_paired_change_percent']=100*(statistics.median(p['ratio'] for p in ps)-1)
 summary.append(row)
data.update(summary=summary,note=note);write_json(out/'ppml/comparisons.json',data)
with (out/'ppml/summary.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(summary[0]));w.writeheader();w.writerows(summary)
allpairs=[]
model_gaps=[]
for model in ['ols','ppml']:
 data=read_json(out/model/'comparisons.json');allpairs.extend((model,p) for p in data['pairs'])
 with zipfile.ZipFile(out/model/'numeric-evidence.zip','a',zipfile.ZIP_DEFLATED) as z:
  for p in sorted((root/'cluster-certificate-data').glob('*.json')):
   z.write(p,'cluster-certificate-inputs/'+p.name)
 a=read_json(out/model/'archive.json');a.update(sha256=sha256(out/model/'numeric-evidence.zip'),primary_pairs=sum(p['group']=='primary' for p in data['pairs']))
 write_json(out/model/'archive.json',a)
 for p in data['pairs']:
  for arm in ['baseline','candidate']:
   if p[arm]['native_peak_bytes']>p[arm]['modeled_bytes']:
    model_gaps.append(dict(model=model,pair=p['pair'],arm=arm,observed=p[arm]['native_peak_bytes'],modeled=p[arm]['modeled_bytes']))
  print('REVIEW',model,p['group'],p['case'],p['baseline']['command_seconds'],p['candidate']['command_seconds'],p['baseline']['native_peak_bytes'],p['candidate']['native_peak_bytes'])
assert len(allpairs)==9
assert {g['pair'].split('-control-')[1].split('-s314')[0] for g in model_gaps}=={'small_slope','weak_pw'}
assert len(model_gaps)==4
write_json(out/'model-gaps.json',dict(gaps=model_gaps,note='Both gaps occur on the accepted baseline and candidate. They are not admitted-peak passes. Native reporting is unchanged; P0 remains open.'))
for model in ['ols','ppml']:
 with zipfile.ZipFile(out/model/'numeric-evidence.zip') as z:
  assert z.testzip() is None
  for name in z.namelist():
   data=z.read(name);assert all(x not in data for x in [b'/Users/',b'/private/',b'/tmp/']),name
print('NINE_PAIRS_CHECKED')
