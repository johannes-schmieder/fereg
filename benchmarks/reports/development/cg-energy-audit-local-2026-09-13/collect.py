from pathlib import Path
import csv,json,re,hashlib,subprocess,zipfile
from common import read_json,write_json,sha256
from run import BASE
from final_compare import matrix
from ppml_preparation_compare import named_runtime
rows=[];pairs=[]
for p in sorted((BASE/'runs').glob('control-*-candidate')):
 q=p.with_name(p.name.removesuffix('-candidate')+'-baseline');a,b=(read_json(x/'result.json') for x in [q,p]);cell=read_json(p/'request.json')['cell'];name=cell['name'];review=read_json(p/'cost-review.json');assert not review['review']
 for arm,path,r in [('baseline',q,a),('candidate',p,b)]:
  if cell['model']=='ppml':
   rt=named_runtime(path);native=rt['allocation_peak_delta'];model=rt['modeled_selected_bytes'];scratch=rt['observed_scratch_bytes'];input_s=None;native_s=rt['total_seconds'];fallback=rt['fallback_count'];iterations=rt['inner_iterations'];setup=None;solve=None
  else:
   rt=matrix(path,'fereg_runtime')[:,1];sl=matrix(path,'fereg_solver');native=r['metadata']['fereg_native_allocation_bytes'];model=r['metadata']['fereg_memory_selected_bytes'];scratch=r['metadata']['fereg_scratch_bytes'];input_s=float(rt[18]);native_s=float(rt[19]);fallback=int(sum(sl[:,10]));iterations=int(sum(sl[:,2]));setup=float(sum(sl[:,8]));solve=float(sum(sl[:,9]))
  rows.append(dict(case=name,arm=arm,seconds=r['seconds'],input_seconds=input_s,native_seconds=native_s,initial_final_setup_seconds=setup,initial_final_solve_seconds=solve,native_bytes=native,modeled_bytes=model,scratch_bytes=scratch,peak_rss_gib=r['peak_rss_gib'],fallback=fallback,iterations=iterations))
 x,y=rows[-2:];assert x['native_bytes']==y['native_bytes'];assert x['modeled_bytes']==y['modeled_bytes'];assert x['scratch_bytes']==y['scratch_bytes']
 pairs.append(dict(case=name,**review,native_peak_exact=True,modeled_peak_exact=True,scratch_exact=True,native_admitted=x['native_bytes']<=x['modeled_bytes'],baseline_seconds=a['seconds'],candidate_seconds=b['seconds']))
with (BASE/'summary.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)
write_json(BASE/'comparisons.json',pairs)
print(json.dumps(pairs,indent=2))
