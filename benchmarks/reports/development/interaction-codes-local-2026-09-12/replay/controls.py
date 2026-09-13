import paired
from paired import ols,ppml
from common import read_json,write_json,sha256
import pandas as pd,numpy as np,datetime,hashlib
old_input=ols.input_for
def input_for(spec,seed,warm=False):
 original=old_input(spec,seed,warm)
 if spec['name'] not in ['bound_admit','bound_decline','sparse_fallback']:return original
 path=ols.BASE/'bounded-interaction-data'/original.name
 if not path.exists():
  path.parent.mkdir(parents=True,exist_ok=True)
  frame=pd.read_stata(original,convert_categoricals=False);i=np.arange(len(frame))
  if spec['name']=='sparse_fallback':
   frame['c1']=i.astype(float);frame['c2']=(i%3).astype(float)
  else:
   frame['c1']=(i%240).astype(float);frame['c2']=((i//120)%256).astype(float)
  frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  info=read_json(original.with_suffix('.json'));hashes={}
  for name in frame:
   values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
  info.update(schema='interaction-budget-input-v1',source_sha256=sha256(original),sha256=sha256(path),column_sha256=hashes,recipe=spec)
  write_json(path.with_suffix('.json'),info)
 return path
ols.input_for=input_for
CONTROLS=[
 ('ols',dict(name='bound_admit',design='one_fe',n=30720,k=10,threads=4,vce='cluster c1#c2')),
 ('ols',dict(name='bound_decline',design='one_fe',n=30720,k=10,threads=4,vce='cluster c1#c2',restriction='if obs_id > 1')),
 ('ols',dict(name='sparse_fallback',design='one_fe',n=30720,k=10,threads=4,vce='cluster c1#c2')),
 *[('ols',dict(name='ols_'+name,design=name,n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1#c2')) for name in ['weak_d3','two_fe_dense','bottleneck_ring','cycle','disconnected_blocks','unbalanced']],
 ('ols',dict(name='small_narrow',design='one_fe',n=7680,k=1,threads=1,weight='[aw=w]',vce='cluster c1#c2')),
 ('ols',dict(name='low_saved_interaction',design='one_fe',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1#c2',absorb='saved_g=g1#c2',fit_options='memory(low) residuals(saved_e)',missing=True)),
 ('ols',dict(name='no_fe_interaction',design='one_fe',n=30720,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2',effects=[])),
 ('ols',dict(name='iid_complete',design='one_fe',n=983040,k=50,threads=16)),
 ('ppml',dict(name='ppml_weak',design='weak_d3',n=30720,k=10,threads=4,weight='[fw=w]',vce='cluster c1#c2',fit_options='offset(off)')),
 ('ppml',dict(name='ppml_saved_separation',design='one_fe',n=7680,k=3,threads=4,weight='[fw=w]',vce='cluster c1#c2',absorb='saved_g=g1#c2',fit_options='offset(off) mu(saved_mu) dropreason(saved_drop)',zero_groups=True)),
]
if __name__=='__main__':
 for j,(model,spec) in enumerate(CONTROLS):
  (ols if model=='ols' else ppml).pair(spec,15485863,rotation=j,prefix='interaction-control-')
