"""Descriptive same-input core comparisons from completed, grouped calls only."""
from pathlib import Path
import hashlib,json,statistics,sys
root,out=map(Path,sys.argv[1:]);records=[]
sys.path.insert(0,str(Path('benchmarks/systematic').resolve()))
from runner import compare
science=json.loads(Path('benchmarks/systematic/manifest-light-v2.json').read_text())['science']
plan=json.loads((root/'plan.json').read_text());count=len(plan['cases'][0]['seeds'])
for low,high in [(1,2),(2,4),(1,4)]:
 for rep in range(1,count+1):
  a=root/f'one-small-t{low}'/f'pair-{rep}';b=root/f'one-small-t{high}'/f'pair-{rep}'
  assert json.loads((a/'input.json').read_text())['sha256']==json.loads((b/'input.json').read_text())['sha256']
  for arm in ('baseline','candidate'):
   p,q=a/arm/'result.json',b/arm/'result.json';u,v=json.loads(p.read_text()),json.loads(q.read_text())
   assert u['status']==v['status']=='pass' and u['sample_sha256']==v['sample_sha256']
   assert u['metadata']['feppml_threads']==low and v['metadata']['feppml_threads']==high
   c=compare(b/arm,a/arm,science);assert c['status']=='pass',c
   records.append(dict(arm=arm,lower_threads=low,higher_threads=high,repetition=rep,higher_over_lower=v['seconds']/u['seconds'],science=c,lower_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),higher_sha256=hashlib.sha256(q.read_bytes()).hexdigest()))
summary=[]
for low,high in [(1,2),(2,4),(1,4)]:
 for arm in ('baseline','candidate'):
  v=[r['higher_over_lower'] for r in records if r['arm']==arm and r['lower_threads']==low and r['higher_threads']==high]
  summary.append(dict(arm=arm,lower_threads=low,higher_threads=high,median=statistics.median(v),values=v,slower=sum(x>1 for x in v)))
out.write_text(json.dumps(dict(scope='Post-hoc comparison of same-input grouped calls. Thread requests were not directly alternating. Descriptive only; no monotonic-scaling claim.',records=records,summary=summary),indent=2)+'\n')
