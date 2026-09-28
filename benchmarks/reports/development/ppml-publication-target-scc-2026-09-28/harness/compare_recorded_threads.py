from pathlib import Path
import json,hashlib,statistics,sys
root,out=map(Path,sys.argv[1:]);records=[]
for rep in (1,2,3):
 a=root/'target-small-serial'/f'pair-{rep}';b=root/'target-profiled-small'/f'pair-{rep}'
 if not a.exists():continue
 x=json.loads((a/'input.json').read_text());y=json.loads((b/'input.json').read_text());assert x['sha256']==y['sha256']
 for arm in ('baseline','candidate'):
  p,q=a/arm/'result.json',b/arm/'result.json';u,v=json.loads(p.read_text()),json.loads(q.read_text())
  assert u['status']==v['status']=='pass' and u['sample_sha256']==v['sample_sha256']
  assert u['metadata']['feppml_threads']==1 and v['metadata']['feppml_threads']==4
  # Standard frozen comparison also checks finite coordinates, SE, VCE and objectives.
  sys.path.insert(0,str(Path('benchmarks/systematic').resolve()))
  from runner import compare
  science=json.loads(Path('benchmarks/systematic/manifest-light-v2.json').read_text())['science']
  c=compare(b/arm,a/arm,science);assert c['status']=='pass',c
  records.append(dict(arm=arm,repetition=rep,higher_over_lower=v['seconds']/u['seconds'],science=c,lower_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),higher_sha256=hashlib.sha256(q.read_bytes()).hexdigest()))
summary={a:dict(median=statistics.median(r['higher_over_lower'] for r in records if r['arm']==a),values=[r['higher_over_lower'] for r in records if r['arm']==a],slower=sum(r['higher_over_lower']>1 for r in records if r['arm']==a)) for a in ('baseline','candidate')}
out.write_text(json.dumps(dict(scope='Post-hoc comparison of same-input grouped T1/T4 calls; thread requests were not directly alternating. Descriptive evidence only, not a monotonic-scaling claim.',records=records,summary=summary),indent=2)+'\n')
print(summary)
