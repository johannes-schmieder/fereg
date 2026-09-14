import os
from pathlib import Path
import json,hashlib,datetime
import numpy as np
import pandas as pd
from preparation_compare import input_for
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original=json.loads((r/'requests.json').read_text())[0]
primary=[dict(name='ols_fast_aw',model='ols',design='one_fe',n=491520,k=8,threads=16,weight='[aw=w]',levels=8,h_levels=8,missing=True),dict(name='generated_unit',model='ols',design='one_fe',n=491520,k=8,threads=4,levels=8,h_levels=8,missing=True),dict(name='generated_pw',model='ols',design='one_fe',n=491520,k=8,threads=16,weight='[pw=w]',vce='cluster c1 c2',levels=8,h_levels=8,missing=True)]
controls=[]
for design in ['weak_d3','bottleneck_ring','cycle','disconnected_blocks','unbalanced']:
 spec=dict(name=design,model='ols',design=design,n=30720,k=2,threads=4,levels=4,h_levels=4)
 if design=='disconnected_blocks':spec['weight']='[fw=w]'
 if design=='unbalanced':spec.update(weight='[aw=w]',vce='robust')
 controls.append(spec)
controls.extend([dict(name='three_fe',design='three_fe_sparse',n=30720,k=8,effects=['g1','g2','g3'],weight='[fw=w]'),dict(name='slope',design='one_fe',n=8193,k=8,absorb='g1##c.z',slope=True),dict(name='saved',design='one_fe',n=30720,k=8,weight='[aw=w]',absorb='saved_g=g1',fit_options='residuals(saved_e)'),dict(name='small_generated',design='one_fe',n=4095,k=8,levels=4,h_levels=8),dict(name='small_numeric',design='one_fe',n=7680,k=2,threads=1,numeric_only=True),dict(name='topology_rank',design='one_fe',n=30720,k=3,absorb='g1 f',effects=['g1','f'],rank_deficient=True),dict(name='width7',design='one_fe',n=8193,k=3,levels=2,h_levels=2),dict(name='width8',design='one_fe',n=8193,k=4,levels=2,h_levels=2),dict(name='weighted_cut',design='weighted_cut',n=16384,k=2,weight='[pw=w]',vce='robust')])
bridge=next(x for x in json.loads(Path(os.path.join(os.environ['SCHUR_CHECKPOINT_ROOT'], 'followup-requests.json')).read_text()) if x['name']=='weighted_cut')
requests=[]
def prepare(spec,seed,group):
 spec=dict(model='ols',threads=4,vce='',levels=4,h_levels=4,**{k:v for k,v in spec.items() if k not in ['model','threads','vce','levels','h_levels']})|{k:v for k,v in spec.items() if k in ['model','threads','vce','levels','h_levels']}
 cell=dict(section='width',**spec);cell.setdefault('effects',['g1'] if spec['design']=='one_fe' else ['g1','g2']);cell['rhs']=f"x1-x{spec['k']}"+('' if spec.get('numeric_only') else ' i.f##i.h')
 row={'name':spec['name']+'-s'+str(seed),'group':group,'seed':seed,'warm_seed':seed+1,'cell':cell}
 for warm,key,actual_seed in [(False,'data',seed),(True,'warm',seed+1)]:
  if spec['design']=='weighted_cut':parent=Path(bridge[key]);assert sha(parent)==bridge[key+'_sha256']
  else:parent=input_for(spec,actual_seed,warm)
  frame=pd.read_stata(parent,convert_categoricals=False);n=len(frame);rng=np.random.default_rng(actual_seed+19);frame['f']=rng.integers(0,spec['levels'],size=n).astype(float);frame['h']=rng.integers(0,spec['h_levels'],size=n).astype(float)
  if spec.get('missing'):frame.loc[np.arange(n)%971==0,'h']=np.nan
  if spec.get('slope'):frame['z']=rng.normal(size=n)
  if spec.get('rank_deficient'):frame['x3']=2*frame.x1
  if spec['name']=='small_generated' and warm:frame=frame.iloc[:4095].copy();n=len(frame)
  path=r/'data'/(row['name']+'-'+key+'.dta');path.parent.mkdir(exist_ok=True);frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1));hashes={}
  for name in frame:
   values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
  info={'source_sha256':sha(parent),'sha256':sha(path),'columns':list(frame),'column_sha256':hashes,'n':n,'seed':actual_seed,'recipe':spec,'f_h_generator':'default_rng(seed+19), sequential f then h, optional missing h every971 rows; see executable recipe'};path.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n');row[key]=str(path);row[key+'_sha256']=sha(path);row[key+'_metadata_sha256']=sha(path.with_suffix('.json'))
 requests.append(row)
for spec in primary:
 for seed in [104729,8675309,20260819]:
  if spec['name']=='ols_fast_aw' and seed==104729:continue
  prepare(spec,seed,'primary')
for spec in controls:prepare(spec,31415927,'control')
assert len(requests)==22;(r/'followup-requests.json').write_text(json.dumps(requests,indent=2)+'\n');print('Prepared8additionalprimaryand14controlrequests',flush=True)
