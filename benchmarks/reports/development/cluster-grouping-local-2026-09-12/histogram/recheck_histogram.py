import compare
from common import sha256, write_json
import numpy as np
import pandas as pd
import datetime
old_input=compare.input_for
def input_for(spec,seed,warm=False):
 p=old_input(spec,seed,warm)
 q=compare.BASE/'cluster-data'/p.name
 if not q.exists():
  q.parent.mkdir(parents=True,exist_ok=True)
  frame=pd.read_stata(p,convert_categoricals=False)
  rng=np.random.default_rng(seed)
  for name,groups in [('c1',97),('c2',13),('c3',7)]:frame[name]=rng.integers(0,groups,size=len(frame)).astype(float)
  frame.to_stata(q,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  hashes={}
  for name in frame:
   v=frame[name].to_numpy(dtype='<f8',copy=True);v[np.isnan(v)]=float.fromhex('0x1p+1023')
   import hashlib
   hashes[name]=hashlib.sha256(v.tobytes()).hexdigest()
  write_json(q.with_suffix('.json'),dict(schema='cluster-histogram-input-v1',source_sha256=sha256(p),sha256=sha256(q),n=len(frame),k=spec['k'],seed=seed,columns=list(frame),column_sha256=hashes,recipe=spec,binary_missing_encoding='Stata system missing (2^1023)'))
 return q
compare.input_for=input_for
CASES=[
 dict(name='two_way_wide',design='one_fe',n=983040,k=50,threads=16,vce='cluster c1 c2',weight='[pw=w]'),
 dict(name='three_way_narrow',design='one_fe',n=983040,k=10,threads=4,vce='cluster c1 c2 c3',weight='[aw=w]'),
]
spec=CASES[0]
for i,seed in enumerate([104729,8675309,20260819]):
 paths=compare.pair(spec,seed,i+1,prefix='histogram-recheck-')
 for name in ['b','V','fereg_fit','fereg_certificate']:
  assert (paths['baseline']/(name+'.csv')).read_bytes()==(paths['candidate']/(name+'.csv')).read_bytes(),name
