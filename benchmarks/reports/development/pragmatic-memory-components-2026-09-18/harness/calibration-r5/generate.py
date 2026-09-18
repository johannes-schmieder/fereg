from pathlib import Path
import hashlib,json,datetime
import numpy as np
import pandas as pd
from common import sha256,write_json
root=Path(__file__).resolve().parents[1]
(root/'data').mkdir(exist_ok=True)
manifest=json.loads((root/'fixed-cases.json').read_text());bindings={}
for req in manifest['cases']:
 n,k=req['cell']['n'],req['cell']['k'];rng=np.random.default_rng(manifest['seed']);i=np.arange(n)
 x=rng.normal(size=(n,k))*.3
 g1=i%128;g2=(i//128+i%17)%64
 graph=req['graph'];w=1+i%3
 if graph=='tiny':g1=(i//64)*2+i%2;g2=(i//64)*2+(i//2)%2
 if graph=='mixed':
  mask=i>=n//2;g1[mask]=128+((i[mask]-n//2)//64)*2+i[mask]%2;g2[mask]=64+((i[mask]-n//2)//64)*2+(i[mask]//2)%2
 if graph=='bridge':
  half=i>=n//2;g1=i%64+half*64;g2=(i//64+i%17)%32+half*32
  g1[-2]=0;g2[-2]=32;g1[-1]=64;g2[-1]=0;w=w.astype(float);w[-2:]=1e-8
 if graph=='three':
  block=i%2;g1=i%32+block*32;g2=(i//32)%16+block*16
 eta=.4+x[:,0]*.15+x[:,1]*.1+np.sin(g1*.07)*.2+np.cos(g2*.11)*.1
 y=(rng.poisson(np.exp(eta))+1).astype(float) if req['cell']['model']=='ppml' else eta+rng.normal(size=n)*.1
 if req['id']=='ppml_separation':y[g1<4]=0
 data={'obs_id':(i+1).astype(float),'y':y}
 for j in range(k):data[f'x{j+1}']=x[:,j]
 if req['id']=='ols_wide_missing':data['x1']=data['x1'].copy();data['x1'][::1000]=np.nan
 data.update(g1=(g1+1).astype(float),g2=(g2+1).astype(float),w=w.astype(float))
 if graph=='three':data['g3']=((i//512)%8+block*8+1).astype(float)
 df=pd.DataFrame(data);path=root/'data'/req['data'];df.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 # Stata encodes system missing as 2^1023 rather than IEEE NaN at its SPI.
 columns={}
 for c in df.columns:
  values=df[c].to_numpy(dtype='<f8').copy();values[np.isnan(values)]=2.0**1023
  columns[c]=hashlib.sha256(values.tobytes()).hexdigest()
 info={'n':n,'k':k,'columns':list(df.columns),'column_sha256':columns,'dta_sha256':sha256(path),'generator_seed':manifest['seed']}
 write_json(path.with_suffix('.json'),info);bindings[req['id']]={'data_sha256':sha256(path),'metadata_sha256':sha256(path.with_suffix('.json'))}
write_json(root/'generated-bindings.json',bindings)
