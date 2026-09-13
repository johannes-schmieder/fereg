from pathlib import Path
import datetime,hashlib,json
import numpy as np
import pandas as pd
import preparation_compare as ols
from common import sha256,write_json
ols.BASELINE='4e18c223a6b741b04bbcee35d10790729f5c8171'
ordinary_input=ols.input_for

def input_for(spec,seed,warm=False):
 p=ordinary_input(spec,seed,warm)
 if spec.get('constant') is None:return p
 frame=pd.read_stata(p,convert_categoricals=False)
 frame.loc[frame.w.notna(),'w']=float(spec['constant'])
 frame.to_stata(p,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 manifest=json.loads(p.with_suffix('.json').read_text())
 hashes={}
 for name in frame:
  values=frame[name].to_numpy(dtype='<f8',copy=True)
  values[np.isnan(values)]=float.fromhex('0x1p+1023')
  hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
 manifest.update(sha256=sha256(p),column_sha256=hashes)
 write_json(p.with_suffix('.json'),manifest)
 return p
ols.input_for=input_for
PRIMARY=[
 dict(name='constant_aw_narrow',design='one_fe',n=983040,k=1,threads=1,weight='[aw=w]',constant=128),
 dict(name='constant_aw_iid',design='one_fe',n=983040,k=50,threads=16,weight='[aw=w]',constant=128),
 dict(name='constant_fw_cluster',design='one_fe',n=245760,k=50,threads=4,weight='[fw=w]',vce='cluster c1',constant=128),
 dict(name='constant_aw_schur',design='strong_d3',n=245760,k=50,threads=4,weight='[aw=w]',constant=128),
]
CONTROLS=[
 dict(name='varying_aw_robust',design='one_fe',n=245760,k=50,threads=16,weight='[aw=w]',vce='robust'),
 dict(name='varying_weak_aw',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='robust'),
]
if __name__=='__main__':
 for i,spec in enumerate(PRIMARY+CONTROLS):ols.pair(spec,104729,i,prefix='constant-pilot-')
