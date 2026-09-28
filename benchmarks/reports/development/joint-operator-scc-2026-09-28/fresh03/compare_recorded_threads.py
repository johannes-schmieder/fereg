from pathlib import Path
import json,sys,statistics
run=Path('/projectnb/welfgr/fereg/dev/20260928-joint-operator-rotation03')
sys.path.insert(0,str(run/'rotation03-harness/systematic'));sys.path.insert(0,str(run/'rotation03-harness/performance'))
from runner import compare
from rotation_threads import native_threads_match
config=json.loads(Path('/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76/input/manifest.json').read_text())['science'];rows=[]
for i in range(1,6):
 left=run/'rotation03-results/fresh-profiled-small'/f'pair-{i}'/'candidate';right=run/'rotation03-results/fresh-small-serial'/f'pair-{i}'/'candidate'
 c=compare(left,right,config);a,b=[json.loads((p/'result.json').read_text()) for p in [left,right]]
 c['checks']['native_rank_dof']=all(a['metadata'].get(k) is not None and a['metadata'].get(k)==b['metadata'].get(k) for k in ['rank','df_a','df_m','df_resid'])
 c['checks']['actual_threads']=native_threads_match(a,4,4) and native_threads_match(b,1,1)
 assert c['status']=='pass' and all(c['checks'].values())
 rows.append(dict(repetition=i,input_sha256=a['input_sha256'],comparison=c,seconds_t1=b['seconds'],seconds_t4=a['seconds'],ratio_t4_t1=a['seconds']/b['seconds']))
out=dict(source_plugin='a7e95e6712c6b05b0217a9ddffa5b8f14c0f8d95443c2b983ce459f16a0e4596',comparisons=rows,median_ratio=statistics.median(r['ratio_t4_t1'] for r in rows),scope='Corresponding repetitions from separately grouped request cases, not directly alternating thread requests; no new fits, unchanged frozen scientific comparator.')
print(json.dumps(out,indent=2))
