"""Source-derived scratch read requests; not measured hardware or filesystem I/O."""
from pathlib import Path
import json,hashlib
import numpy as np
import pandas as pd
root=Path(__file__).parent
out=[]
for case,k,dimensions in [('low_fw_cluster',10,['c1']),('low_pw_multiway',16,['c1','c2'])]:
 p=next((root/'preparation-data').glob(f'{case}-n*-s104729.dta'))
 frame=pd.read_stata(p,convert_categoricals=False)
 required=['y','g1','w',*[f'x{i}' for i in range(1,k+1)],*dimensions]
 active=frame.dropna(subset=required)
 sample=hashlib.sha256(active.obs_id.to_numpy(dtype='<u8').tobytes()).hexdigest()
 receipt=json.loads((root/'runs'/f'inference-low-paired-{case}-s104729-baseline/result.json').read_text())
 assert sample==receipt['sample_sha256']
 n=len(active);capacity=(1<<20)//(8*k)
 masks=sorted(range(1,1<<len(dimensions)),key=lambda m:(m.bit_count(),m))
 result=dict(case=case,rows=n,rank=k,tile_rows=capacity,sample_sha256=sample,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),initial_cache='empty before covariance; persisted across subsets')
 for arm in ['baseline','candidate']:
  start=rows=0;reads=bytes_=0
  for mask in masks:
   keys=[active[col].to_numpy() for j,col in enumerate(dimensions) if mask&(1<<j)]
   order=np.lexsort(tuple([np.arange(n),*reversed(keys)])) if arm=='baseline' else range(n)
   for value in order:
    row=int(value)
    if row<start or row>=start+rows:
     start=row;rows=min(capacity,n-row);reads+=1;bytes_+=rows*k*8
  result[arm]=dict(read_requests=reads,requested_bytes=bytes_,complete_score_passes=len(masks))
 out.append(result)
(root/'read-request-replay.json').write_text(json.dumps(dict(method='Replay of current forward tile-cache policy over sample-hash-verified synthetic rows. Software request calculation, not instrumented read counts or physical disk traffic.',results=out),indent=2)+'\n')
print(json.dumps(out,indent=2))
