from pathlib import Path
import sys,json,zipfile,bisect
from collect_preparation import collect as collect_ols
from collect_ppml_preparation import collect as collect_ppml
from common import read_json,write_json,sha256
root=Path(sys.argv[1]);out=Path(sys.argv[2])
note='Bounded local development. Three rotated primary seeds, separate public traced pilots, nine single-pair controls and a separately reported three-seed PPML two-way recheck. Preserve the original slowdown. No private allocationaudit profile; native/model from result wire, RSS from process sampling. No universal or qualification claim.'
collect_ols(root,out/'ols',pair_prefix='prep-boundaries',primary_cases=['ols_skew'],note=note)
collect_ppml(root,out/'ppml',prefix='ppmlprep-boundaries',primary_cases={'ppml_skew'})
p=out/'ppml/comparisons.json';d=read_json(p);d['note']=note;write_json(p,d)
for model in ['ols','ppml']:
 with zipfile.ZipFile(out/model/'numeric-evidence.zip','a',zipfile.ZIP_DEFLATED) as archive:
  for p in sorted((root/('skew-'+model+'-data')).glob('*.json')):archive.write(p,'skew-inputs/'+p.name)
 p=out/model/'archive.json';d=read_json(p);d['sha256']=sha256(out/model/'numeric-evidence.zip');write_json(p,d)
 for r in read_json(out/model/'comparisons.json')['summary']:
  print('DETAIL',model,json.dumps(r))
 for r in read_json(out/model/'comparisons.json')['pairs']:
  for arm in ['baseline','candidate']:
   v=r[arm]
   if v['native_peak_bytes']>v['modeled_bytes']:print('MODEL GAP',r['pair'],arm,v['native_peak_bytes'],v['modeled_bytes'])
n=983040;ends=[884736+j*1024 for j in range(97)];chunks=60
comparisons=0;extended=0
for j in range(1,chunks):
 b=j*n//chunks;end=ends[bisect.bisect_right(ends,b-1)]
 comparisons+=end-b+int(end<n);extended+=end>b
write_json(out/'logical-boundary-work.json',dict(schema='source-derived-boundary-work-v1',physical_rows=n,cluster_groups=97,giant_group_rows=884736,other_group_rows=1024,chunks=chunks,provisional_boundaries=chunks-1,extended_boundaries=extended,old_adjacent_group_comparisons=comparisons,new_group_comparisons_upper_bound=extended*22+(chunks-1-extended),note='Logical comparison count from the exact complete primary recipe and current 16384-row/64-block policies. The new bound uses the focused <=22 comparisons test for up to 2^20 rows. Not instrumented command counters, physical bytes or hardware traffic. Group reduction, row sorting and chunk count remain.'))
print('COMPLETED')
