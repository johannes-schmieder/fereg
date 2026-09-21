from pathlib import Path
import hashlib,json
import numpy as np
import pandas as pd
from common import sha256,write_json
root=Path(__file__).resolve().parents[1]
(root/'data').mkdir(exist_ok=True)
bindings={}
for req in json.loads((root/'requests.json').read_text()):
 if not req.get('generated'):continue
 n,k=req['cell']['n'],req['cell']['k'];rng=np.random.default_rng(2026091601);i=np.arange(n);groups=256
 data={'obs_id':(i+1).astype(float)}
 x=rng.normal(size=(n,k))*.3
 eta=.4+x[:,0]*.15+x[:,1]*.1+np.sin((i%groups)*.07)*.2
 data['y']=(rng.poisson(np.exp(eta))+1).astype(float)
 for j in range(k):data[f'x{j+1}']=x[:,j]
 data['g1']=(i%groups+1).astype(float)
 if len(req['cell']['effects'])>1:data['g2']=((i//groups+i%17)%128+1).astype(float)
 data['w']=(1+i%3).astype(float)
 df=pd.DataFrame(data);path=root/'data'/req['data'];df.to_stata(path,write_index=False,version=118,time_stamp=__import__("datetime").datetime(2000,1,1))
 info={'n':n,'k':k,'columns':list(df.columns),'column_sha256':{c:hashlib.sha256(df[c].to_numpy(dtype='<f8').tobytes()).hexdigest() for c in df.columns},'dta_sha256':sha256(path),'generator_seed':2026091601}
 write_json(path.with_suffix('.json'),info)
 bindings[req['id']]={'data_sha256':sha256(path),'metadata_sha256':sha256(path.with_suffix('.json'))}
write_json(root/'generated-bindings.json',bindings)
