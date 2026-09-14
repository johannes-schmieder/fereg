import os
from pathlib import Path
import json,hashlib,datetime,shutil
import numpy as np
import pandas as pd
from preparation_compare import input_for as ols_input
from ppml_preparation_compare import input_for as ppml_input
r=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();h=r/'harness';h.mkdir(exist_ok=False)
s=(repo/'benchmarks/local_fixest_gap/measure.py').read_text();s=s.replace('from common import HERE, read_json, sha256, thread_environment, write_json','from common import HERE, read_json, sha256, thread_environment, write_json\nHERE = Path(__file__).resolve().parent');s=s.replace('"fit_options", "string_columns"))','"fit_options", "string_columns", "rhs"))');(h/'measure.py').write_text(s)
s=(repo/'benchmarks/local_fixest_gap/stata_worker.do').read_text().replace('fit_options string_columns\n','fit_options string_columns rhs\n').replace('local effects\n','if "`rhs\'" != "" local xvars `rhs\'\nlocal effects\n',1);(h/'stata_worker.do').write_text(s)
specs=[dict(name='ols_fast_aw',model='ols',design='one_fe',n=491520,k=8,threads=16,weight='[aw=w]',levels=8,missing=True),dict(name='ols_low_pw',model='ols',design='one_fe',n=122880,k=8,threads=4,weight='[pw=w]',vce='cluster c1',fit_options='memory(low)',levels=4,missing=True),dict(name='ppml_fw',model='ppml',design='one_fe',n=122880,k=8,threads=4,weight='[fw=w]',vce='robust',fit_options='exposure(exposure)',levels=4,missing=True)]
requests=[]
for spec in specs:
 row={'name':spec['name'],'seed':104729,'warm_seed':104730,'cell':dict(section='width',effects=['g1'],rhs='x1-x8 i.f##i.h',**spec)}
 for warm,key,seed in [(False,'data',104729),(True,'warm',104730)]:
  original=(ppml_input if spec['model']=='ppml' else ols_input)(spec,seed,warm)
  frame=pd.read_stata(original,convert_categoricals=False);rng=np.random.default_rng(seed+19);n=len(frame);frame['f']=rng.integers(0,spec['levels'],size=n).astype(float);frame['h']=rng.integers(0,8,size=n).astype(float)
  frame.loc[np.arange(n)%971==0,'h']=np.nan
  path=r/'data'/(spec['name']+'-'+key+'.dta');path.parent.mkdir(exist_ok=True);frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  hashes={}
  for name in frame:
   values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
  info={'source_sha256':sha(original),'sha256':sha(path),'columns':list(frame),'column_sha256':hashes,'n':n,'seed':seed,'recipe':spec,'f_h_generator':'numpy default_rng(seed+19), f uniform requested levels, h uniform 8, h missing every 971 rows'};path.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n');row[key]=str(path);row[key+'_sha256']=sha(path);row[key+'_metadata_sha256']=sha(path.with_suffix('.json'))
 requests.append(row)
(r/'requests.json').write_text(json.dumps(requests,indent=2)+'\n');(r/'harness-identities.json').write_text(json.dumps({p.name:sha(p) for p in h.iterdir()},indent=2)+'\n');print('Prepared three fixed screen requests',flush=True)
