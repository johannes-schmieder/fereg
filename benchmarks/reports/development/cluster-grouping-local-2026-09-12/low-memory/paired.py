import compare
CASES=[
 dict(name='low_fw_cluster',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='memory(low)',missing=True),
 dict(name='low_pw_multiway',design='one_fe',n=245760,k=16,threads=16,weight='[pw=w]',vce='cluster c1 c2',fit_options='memory(low)',missing=True),
]
CONTROLS=[
 dict(name='low_small',design='one_fe',n=7680,k=1,threads=1,weight='[aw=w]',vce='cluster c1',fit_options='memory(low)'),
 dict(name='low_weak',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1',fit_options='memory(low)'),
 dict(name='low_high_cardinality',design='one_fe',n=30720,k=10,threads=4,vce='cluster c1 c2',fit_options='memory(low)'),
 dict(name='fast_cluster',design='one_fe',n=245760,k=50,threads=16,vce='cluster c1'),
 dict(name='low_saved',design='one_fe',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1',absorb='saved_g=g1',fit_options='memory(low) residuals(saved_e)',missing=True),
 dict(name='low_no_fe',design='one_fe',n=30720,k=10,threads=4,vce='cluster c1',effects=[],fit_options='memory(low)'),
]
old_input=compare.input_for
def input_for(spec,seed,warm=False):
 p=old_input(spec,seed,warm)
 if spec['name']!='low_high_cardinality':return p
 import pandas as pd, numpy as np, datetime, hashlib
 from common import write_json,sha256
 q=compare.BASE/'high-cardinality-data'/p.name
 if not q.exists():
  q.parent.mkdir(parents=True,exist_ok=True);f=pd.read_stata(p,convert_categoricals=False)
  i=np.arange(len(f));f['c1']=i.astype(float);f['c2']=(i%31).astype(float)
  f.to_stata(q,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  hashes={}
  for name in f:
   v=f[name].to_numpy(dtype='<f8',copy=True);v[np.isnan(v)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(v.tobytes()).hexdigest()
  write_json(q.with_suffix('.json'),dict(schema='low-cluster-cardinality-v1',source_sha256=sha256(p),sha256=sha256(q),n=len(f),k=spec['k'],seed=seed,columns=list(f),column_sha256=hashes,recipe=spec,binary_missing_encoding='Stata system missing (2^1023)'))
 return q
compare.input_for=input_for
if __name__=='__main__':
 for j,spec in enumerate(CASES):
  for i,seed in enumerate([104729,8675309,20260819]):
   paths=compare.pair(spec,seed,j+i,prefix='low-paired-')
   for name in ['b','V','fereg_fit','fereg_certificate']:
    assert (paths['baseline']/(name+'.csv')).read_bytes()==(paths['candidate']/(name+'.csv')).read_bytes(),name
 for j,spec in enumerate(CONTROLS):
  paths=compare.pair(spec,31415927,j,prefix='low-control-')
  for name in ['b','V','fereg_fit','fereg_certificate']:
   assert (paths['baseline']/(name+'.csv')).read_bytes()==(paths['candidate']/(name+'.csv')).read_bytes(),name
