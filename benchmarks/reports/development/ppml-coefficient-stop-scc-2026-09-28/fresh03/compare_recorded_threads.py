"""Compare recorded T4/T1 requests only; these were not alternated directly."""
from pathlib import Path
import hashlib,json,sys,statistics
run=Path(sys.argv[1]);benchmarks=Path(__file__).resolve().parents[4]
sys.path[:0]=[str(benchmarks/'systematic'),str(benchmarks/'performance')]
from runner import compare
from rotation_threads import native_threads_match
identity=json.loads((run/'identity.json').read_text())
for name in ['systematic/runner.py','performance/rotation_threads.py']:
 assert hashlib.sha256((benchmarks/name).read_bytes()).hexdigest()==identity['harness'][name]
config=dict(coefficient_rtol=1e-7,coefficient_atol=1e-10,vce_rtol=1e-6,vce_atol=1e-10,objective_rtol=1e-8,objective_atol=1e-10)
rows=[]
for i in range(1,4):
 left=run/'stop-profiled-small'/f'pair-{i}'/'candidate';right=run/'stop-small-serial'/f'pair-{i}'/'candidate'
 c=compare(left,right,config);a,b=[json.loads((p/'result.json').read_text()) for p in [left,right]]
 c['checks']['native_rank_dof']=all(a['metadata'].get(k) is not None and a['metadata'].get(k)==b['metadata'].get(k) for k in ['rank','df_a','df_m','df_resid'])
 c['checks']['actual_threads']=native_threads_match(a,4,4) and native_threads_match(b,1,1)
 assert c['status']=='pass' and all(c['checks'].values())
 rows.append(dict(repetition=i,input_sha256=a['input_sha256'],comparison=c,seconds_t1=b['seconds'],seconds_t4=a['seconds'],ratio_t4_t1=a['seconds']/b['seconds']))
print(json.dumps(dict(source_plugin=identity['sources']['candidate']['plugin'],comparisons=rows,median_ratio=statistics.median(r['ratio_t4_t1'] for r in rows),scope='Corresponding repetitions from separately grouped request cases, not directly alternating thread requests; no new fits; unchanged frozen science checks.'),indent=2))
