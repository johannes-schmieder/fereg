import datetime
import hashlib
import numpy as np
import pandas as pd
import compare
from common import read_json,sha256,write_json
from preparation_compare import input_for
from run import BASE

def qr_input(spec,seed,warm=False):
 original=input_for(spec,seed,warm)
 path=BASE/'inference-data'/original.name
 if path.exists():return path
 frame=pd.read_stata(original,convert_categoricals=False)
 frame['x2']=frame.x1
 path.parent.mkdir(parents=True,exist_ok=True)
 frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 info=read_json(original.with_suffix('.json'))
 hashes={}
 for name in frame:
  values=frame[name].to_numpy(dtype='<f8',copy=True)
  values[np.isnan(values)]=float.fromhex('0x1p+1023')
  hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
 info.update(source_sha256=sha256(original),sha256=sha256(path),column_sha256=hashes,recipe=dict(spec,exact_duplicate='x2=x1'))
 write_json(path.with_suffix('.json'),info)
 return path

compare.input_for=qr_input
spec=dict(name='qr_collinear',design='one_fe',n=98304,k=50,threads=4,vce='robust',weight='[aw=w]',missing=True)
for r,seed in enumerate([104729,8675309,20260819]):compare.pair(spec,seed,r,trace=True,prefix='profile-')
