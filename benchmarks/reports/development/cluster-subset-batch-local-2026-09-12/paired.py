import compare
CASES=[
 dict(name='low_fw_threeway',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1 c2 c3',fit_options='memory(low)',missing=True),
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
 if spec['name'] not in ['low_high_cardinality','low_fw_threeway','low_manyway']:return p
 import pandas as pd, numpy as np, datetime, hashlib
 from common import write_json,sha256
 q=compare.BASE/'high-cardinality-data'/p.name
 if not q.exists():
  q.parent.mkdir(parents=True,exist_ok=True);f=pd.read_stata(p,convert_categoricals=False)
  i=np.arange(len(f))
  if spec['name']=='low_high_cardinality':f['c1']=i.astype(float);f['c2']=(i%31).astype(float)
  elif spec['name']=='low_fw_threeway':f['c3']=((i//11)%7).astype(float)
  else:
   for j,g in enumerate([7,3,3,2,2]):f['c'+str(j+1)]=((i//(2*j+1))%g).astype(float)
  f.to_stata(q,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  hashes={}
  for name in f:
   v=f[name].to_numpy(dtype='<f8',copy=True);v[np.isnan(v)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(v.tobytes()).hexdigest()
  write_json(q.with_suffix('.json'),dict(schema='low-cluster-cardinality-v1',source_sha256=sha256(p),sha256=sha256(q),n=len(f),k=spec['k'],seed=seed,columns=list(f),column_sha256=hashes,recipe=spec,binary_missing_encoding='Stata system missing (2^1023)'))
 return q
compare.input_for=input_for

CONTROLS.extend([
 dict(name='low_oneway',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='memory(low)',missing=True),
 dict(name='low_manyway',design='one_fe',n=30720,k=4,threads=4,vce='cluster c1 c2 c3 c4 c5',fit_options='memory(low)'),
 dict(name='weak_multiway',design='weak_d3',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1 c2',fit_options='memory(low)'),
])
if __name__=='__main__':
 import argparse
 parser=argparse.ArgumentParser();parser.add_argument('--part',choices=['pilot','primary','controls'],required=True);args=parser.parse_args()
 specs=CASES if args.part!='controls' else CONTROLS
 for j,spec in enumerate(specs):
  if args.part=='pilot' and j!=1:continue
  seeds=[104729,8675309,20260819] if args.part=='primary' else [31415927]
  for i,seed in enumerate(seeds):
   paths=compare.pair(spec,seed,j+i,trace=args.part=='pilot',prefix='batch-v2-'+args.part+'-')
   for name in ['b','V','fereg_fit','fereg_certificate','saved_outputs']:
    a,b=paths['baseline']/(name+'.csv'),paths['candidate']/(name+'.csv')
    if a.exists():assert a.read_bytes()==b.read_bytes(),name
