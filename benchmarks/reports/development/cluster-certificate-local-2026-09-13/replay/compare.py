"""Bounded preparation-certificate pairs; shared workers and science checkers."""
import argparse,copy,datetime,hashlib
import numpy as np
import pandas as pd
from common import read_json,write_json,sha256
from measure import measure
from run import BASE,MANIFEST
import preparation_compare as ols
import ppml_preparation_compare as ppml
BASELINE=(BASE/'baseline-commit.txt').read_text().strip()
ols.BASELINE=ppml.BASELINE=BASELINE
PRIMARY=[
 ('ols',dict(name='ols_dense',design='one_fe',n=245760,k=50,threads=16,weight='[fw=w]',vce='cluster c1#c2',absorb='g1#c2',missing=True)),
 ('ols',dict(name='ols_sparse',design='one_fe',n=245760,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2',absorb='g1#c2',missing=True,cluster_layout='sparse')),
 ('ppml',dict(name='ppml_sparse',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1#c2',absorb='g1#c2',fit_options='offset(off)',missing=True,cluster_layout='sparse')),
]
CONTROLS=[
 ('ols',dict(name='iid_complete',design='one_fe',n=983040,k=50,threads=16)),
 ('ols',dict(name='simple_cluster',design='one_fe',n=30720,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2')),
 ('ols',dict(name='small_slope',design='one_fe',n=7680,k=2,threads=1,weight='[aw=w]',vce='cluster c1#c2',absorb='g1##c.x1')),
 ('ols',dict(name='low_saved',design='one_fe',n=30720,k=10,threads=4,weight='[aw=w]',vce='cluster c1#c2',absorb='saved_g=g1#c2',fit_options='memory(low) residuals(saved_e)',missing=True)),
 ('ols',dict(name='weak_pw',design='weak_d3',n=30720,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2')),
 ('ppml',dict(name='ppml_dense',design='one_fe',n=122880,k=10,threads=4,weight='[fw=w]',vce='cluster c1#c2',absorb='g1#c2',fit_options='offset(off)',missing=True)),
 ('ppml',dict(name='ppml_saved_separation',design='one_fe',n=7680,k=3,threads=4,weight='[fw=w]',vce='cluster c1#c2',absorb='saved_g=g1#c2',fit_options='offset(off) mu(saved_mu) dropreason(saved_drop)',zero_groups=True)),
 ('ppml',dict(name='ppml_three',design='three_fe_sparse',n=7680,k=10,threads=4,weight='[pw=w]',vce='cluster c1#c2',fit_options='offset(off)')),
]
original_input={'ols':ols.input_for,'ppml':ppml.input_for}
def input_for(model,spec,seed,warm=False):
 original=original_input[model](spec,seed,warm)
 if spec.get('cluster_layout')!='sparse':return original
 path=BASE/'cluster-certificate-data'/original.name
 if not path.exists():
  path.parent.mkdir(parents=True,exist_ok=True)
  frame=pd.read_stata(original,convert_categoricals=False);missing=frame.c1.isna().to_numpy()
  frame['c1']=np.random.default_rng(seed+918273).permutation(len(frame)).astype(float)
  frame.loc[missing,'c1']=np.nan
  frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  info=read_json(original.with_suffix('.json'));hashes={}
  for name in frame:
   values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
  info.update(schema='cluster-certificate-input-v1',source_sha256=sha256(original),sha256=sha256(path),column_sha256=hashes,recipe=spec)
  write_json(path.with_suffix('.json'),info)
 return path
ols.input_for=lambda spec,seed,warm=False:input_for('ols',spec,seed,warm)
def pair(model,spec,seed,rotation=0,trace=False,prefix=''):
 spec=dict(spec);spec.setdefault('effects',MANIFEST['designs'].get(spec['design'],{}).get('effects',['g1','g2']))
 if model=='ols':return ols.pair(spec,seed,rotation,trace,prefix='clcert-'+prefix)
 data,warm=input_for(model,spec,seed),input_for(model,spec,seed+1,True)
 cell=dict(model='ppml',section='width',**spec);paths={}
 for arm in (['candidate','baseline'] if rotation%2 else ['baseline','candidate']):
  source=BASE/arm;path=BASE/'runs'/f'ppmlprep-clcert-{prefix}{spec["name"]}-s{seed}-{arm}';paths[arm]=path
  manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
  result=measure(BASE,cell,'feppml','warm',data,warm,path,manifest,identity=dict(base_commit=BASELINE,variant=arm,seed=seed,trace=trace,plugin_sha256=sha256(source/'dist/fereg_macos.plugin')),source=source,trace=trace)
  print(path.name,result['status'],result['seconds'],flush=True)
  if result['status']!='pass':raise RuntimeError(path)
 ppml.compare(paths)
 return paths
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('part',choices=['screen','repeat','controls','profile']);p.add_argument('--name');a=p.parse_args()
 if a.part in ['screen','repeat']:
  for j,(m,s) in enumerate(PRIMARY):
   if a.name and a.name!=s['name']:continue
   for i,seed in enumerate([104729] if a.part=='screen' else [8675309,20260819]):pair(m,s,seed,j+i+(a.part=='repeat'))
 elif a.part=='controls':
  for i,(m,s) in enumerate(CONTROLS):pair(m,s,31415927,i,prefix='control-')
 else:
  for i,(m,s) in enumerate(PRIMARY[1:]):pair(m,s,67280421,i,True,prefix='profile-')
