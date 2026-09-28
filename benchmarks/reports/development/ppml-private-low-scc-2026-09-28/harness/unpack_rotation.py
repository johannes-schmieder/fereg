from pathlib import Path
import json,hashlib,subprocess,sys
num=sys.argv[1];bundle=json.loads(Path('/tmp/fereg-private-low-rotation'+num+'-bundle.json').read_text());report=Path('benchmarks/reports/development/ppml-private-low-scc-2026-09-28')
sha=lambda b:hashlib.sha256(b).hexdigest()
for n,block in bundle['blocks'].items():
 raw=Path('/tmp/fereg-private-low-rotation'+n+'-raw');raw.mkdir(exist_ok=True);name={'03':'initial03','04':'fresh04','05':'scaling05'}[n];out=report/name;out.mkdir(exist_ok=True)
 for f,d in block['files'].items():
  assert sha(d['text'].encode())==d['sha256'],f
  rel=Path(f);assert not rel.is_absolute() and '..' not in rel.parts
  p=raw/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(d['text'])
  if f in ['plan.json','identity.json','progress.json','complete.json']:(out/f).write_text(d['text'])
 for f,d in block['summary'].items():
  assert sha(d['text'].encode())==d['sha256'];(out/f).write_text(d['text'])
 # Recompute eligibility/ratios from collected receipts and preserve remote bytes.
 subprocess.run(['python3','benchmarks/performance/summarize_rotations.py',str(raw),'--output',str(raw/'recomputed')],check=True,stdout=subprocess.DEVNULL)
 assert json.loads((raw/'recomputed/summary.json').read_text())==json.loads((out/'summary.json').read_text())
 s=json.loads((out/'summary.json').read_text());assert s['complete']
 receipt=dict(job_id=bundle['job'],accounting=bundle['accounting'],job_log_sha256=bundle['job_log_sha256'],sentinels=bundle['sentinels'],no_compilation_profiling_archiving_or_competing_fit_overlap=True,applications=sum(s['statuses'].values()),statuses=s['statuses'],collected_remote_files={f:d['sha256'] for f,d in block['files'].items()},summary_remote_files={f:d['sha256'] for f,d in block['summary'].items()},collection='Every collected byte matches remote SHA256; independent local summary is semantically equal. Raw licensed logs remain remote. Synthetic detailed output retained at the recorded remote root.')
 (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 eq=[]
 for case in s['cases']:
  for rep in range(1,len(case['pairs'])+1):
   d=raw/case['case']/f'pair-{rep}';a=json.loads((d/'baseline/result.json').read_text());b=json.loads((d/'candidate/result.json').read_text())
   fields=['ic','inner_iterations','feppml_strict_profile_rejections','feppml_memory_selected_bytes','feppml_allocation_peak']
   eq.append(dict(case=case['case'],repetition=rep,coefficients_byte_equal=(d/'baseline/coefficients.csv').read_bytes()==(d/'candidate/coefficients.csv').read_bytes(),covariance_byte_equal=(d/'baseline/covariance.csv').read_bytes()==(d/'candidate/covariance.csv').read_bytes(),telemetry={f:dict(baseline=a['metadata'].get(f),candidate=b['metadata'].get(f)) for f in fields if f in a['metadata']}))
 (out/'native-equality.json').write_text(json.dumps(eq,indent=2)+'\n')
 subprocess.run(['python3','benchmarks/reports/development/one-fe-portable-scc-2026-09-28/harness/summarize_phases.py',str(raw),str(out/'phases.json')],check=True)
 print(name,s['statuses'])
 for case in s['cases']:
  def median(ref):
   d=case['comparisons'][ref]['seconds_ratio'];return round(d['median'],3) if d else 'NR'
  print(case['case'],median('baseline'),median('stock'))
