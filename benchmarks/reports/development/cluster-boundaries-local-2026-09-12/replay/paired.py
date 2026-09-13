import argparse,datetime,hashlib
import numpy as np,pandas as pd
import preparation_compare as ols
import ppml_preparation_compare as ppml
from common import read_json,write_json,sha256
ols.BASELINE=ppml.BASELINE='fdf46244aa4d286c1d4ac30565236458ed1ef648'
originals={'ols':ols.input_for,'ppml':ppml.input_for}
def input_for(model,spec,seed,warm=False):
 original=originals[model](spec,seed,warm)
 if not spec.get('skew'):return original
 path=ols.BASE/('skew-'+model+'-data')/original.name
 if path.exists():return path
 path.parent.mkdir(parents=True,exist_ok=True)
 frame=pd.read_stata(original,convert_categoricals=False);i=np.arange(len(frame))
 frame['c1']=np.where(i%10!=0,0,1+(i//10)%96).astype(float)
 if spec.get('missing'):frame.loc[i%983==0,'c1']=np.nan
 frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 info=read_json(original.with_suffix('.json'));hashes={}
 for name in frame:
  values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
 info.update(schema='cluster-skew-input-v1',source_sha256=sha256(original),sha256=sha256(path),column_sha256=hashes,recipe=spec)
 write_json(path.with_suffix('.json'),info)
 return path
ols.input_for=lambda spec,seed,warm=False:input_for('ols',spec,seed,warm)
ppml.input_for=lambda spec,seed,warm=False:input_for('ppml',spec,seed,warm)
PRIMARY=[
 ('ols',dict(name='ols_skew',design='one_fe',n=983040,k=10,threads=4,weight='[pw=w]',vce='cluster c1',skew=True)),
 ('ppml',dict(name='ppml_skew',design='one_fe',n=983040,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='offset(off)',skew=True)),
]
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--part',choices=['pilot','primary'],required=True);args=p.parse_args()
 for j,(model,spec) in enumerate(PRIMARY):
  for i,seed in enumerate([104729,8675309,20260819] if args.part=='primary' else [31415927]):
   (ols if model=='ols' else ppml).pair(spec,seed,j+i,trace=args.part=='pilot',prefix='boundaries-'+args.part+'-')
