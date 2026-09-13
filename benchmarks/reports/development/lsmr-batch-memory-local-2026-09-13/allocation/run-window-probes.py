import json,subprocess,hashlib,os
from pathlib import Path
import numpy as np
root=Path(os.environ['FEREG_LSMR_PROBE_OUTPUT'])
root.mkdir(parents=True,exist_ok=False)
binary=Path(os.environ['FEREG_LSMR_PROBE_BINARY'])
records=[]
# Separate exact-chain witness for the weighted memory-routing regression.
chain=[320,1,1,160,161,'schwarz',1,0,1,'chain']
r=subprocess.run([str(binary),*map(str,chain)],capture_output=True,text=True)
(root/'chain-allocation.json').write_text(json.dumps({'args':chain,'rc':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()},indent=2)+'\n')
assert r.returncode==0,r.stderr
print('chain',r.stdout,flush=True)
for repeat,order in enumerate([[67,4,8,16],[16,8,4,67],[8,67,16,4]]):
 for window in order:
  output=root/f'window-{repeat}-{window}.bin'
  args=[122880,67,4,27927,27629,'schwarz',1,1,window]
  r=subprocess.run([str(binary),*map(str,args)],env={**os.environ,'FEREG_PROBE_OUTPUT':str(output)},capture_output=True,text=True)
  record={'repeat':repeat,'window':window,'args':args,'rc':r.returncode,'stderr':r.stderr,'measurement':json.loads(r.stdout) if r.returncode==0 else r.stdout}
  records.append(record)
  (root/'window-results.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'records':records},indent=2)+'\n')
  print(json.dumps(record),flush=True)
  assert r.returncode==0,r.stderr
 for window in [4,8,16]:
  baseline=np.fromfile(root/f'window-{repeat}-67.bin',dtype='<f8')
  candidate=np.fromfile(root/f'window-{repeat}-{window}.bin',dtype='<f8')
  assert baseline.shape==candidate.shape
  a=next(x for x in records if x['repeat']==repeat and x['window']==window)
  a['full_values']={'count':len(baseline),'exact':bool(np.array_equal(baseline,candidate)),'maximum_absolute_difference':float(np.max(np.abs(baseline-candidate))),'baseline_sha256':hashlib.sha256(baseline.tobytes()).hexdigest(),'candidate_sha256':hashlib.sha256(candidate.tobytes()).hexdigest(),'within_1e12_tolerance':bool(np.allclose(baseline,candidate,rtol=1e-12,atol=1e-12))}
  assert a['full_values']['within_1e12_tolerance'],a['full_values']
 (root/'window-results.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'records':records},indent=2)+'\n')
