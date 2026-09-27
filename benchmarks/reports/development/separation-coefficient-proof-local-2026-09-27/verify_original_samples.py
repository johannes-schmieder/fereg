"""Independent row-set verification from repository root, using preserved proofs."""
from pathlib import Path
import hashlib,json
import numpy as np
import pandas as pd
repo=Path.cwd();root=repo/'target/performance-repair-20260927'
proofs=json.loads((repo/'benchmarks/reports/development/separation-graph-repair-local-2026-09-27/separation-graph-results.json').read_text())
records=[]
for proof in proofs:
 case=proof['case'];data=root/'original-zero-data'/(case+'.dta')
 assert hashlib.sha256(data.read_bytes()).hexdigest()==proof['input_sha256']
 frame=pd.read_stata(data,convert_categoricals=False)
 keep=~frame.obs_id.isin(proof['separated_obs_id']).to_numpy()
 effects=[name for name in ('g1','g2','g3') if name in frame]
 while True:
  rows=np.flatnonzero(keep);active=frame.iloc[rows]
  retained=np.ones(len(rows),dtype=bool)
  for effect in effects:
   grouped=active.groupby(effect).y
   retained &= (grouped.transform('size').to_numpy()>1)&(grouped.transform('sum').to_numpy()>0)
  if retained.all():break
  keep[rows[~retained]]=False
 native=root/'coefficient-proof04-probes'/case/'sample.csv'
 observed=pd.read_csv(native)
 assert np.array_equal(observed.obs_id.to_numpy(),frame.obs_id.to_numpy())
 assert np.array_equal(observed.audit_sample.to_numpy().astype(bool),keep)
 rows=frame.obs_id.to_numpy()[keep].astype('<i8')
 records.append({'case':case,'input_sha256':proof['input_sha256'],'exact_proven_separation_rows':proof['separated_rows'],'subsequent_singleton_or_zero_level_rows':int((~keep).sum())-proof['separated_rows'],'retained_rows':len(rows),'retained_original_ids_sha256':hashlib.sha256(rows.tobytes()).hexdigest(),'native_sample_file_sha256':hashlib.sha256(native.read_bytes()).hexdigest(),'exact_row_match':True})
out=repo/'benchmarks/reports/development/separation-coefficient-proof-local-2026-09-27/candidate04-sample-verification-replay.json'
with out.open('x') as stream: stream.write(json.dumps(records,indent=2)+'\n')
print(json.dumps(records,indent=2))
