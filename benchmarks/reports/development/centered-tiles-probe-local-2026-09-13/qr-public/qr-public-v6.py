import argparse,copy,datetime,hashlib,json
import numpy as np
import pandas as pd
from common import sha256,write_json,read_json
from run import BASE,MANIFEST
from preparation_compare import input_for
from final_compare import baseline_compare,certify
from measure import measure
CASES=[
 dict(name='near_unit_iid',design='one_fe',n=16384,k=16,threads=4,near_capacity=True),
 dict(name='near_aw_robust',design='one_fe',n=16384,k=16,threads=4,near_capacity=True,weight='[aw=w]',vce='robust'),
 dict(name='near_fw_cluster',design='one_fe',n=16384,k=16,threads=4,near_capacity=True,weight='[fw=w]',vce='cluster c1'),
 dict(name='near_pw_cluster',design='one_fe',n=16384,k=16,threads=4,near_capacity=True,weight='[pw=w]',vce='cluster c1 c2'),
 dict(name='wide_deficient_aw',design='one_fe',n=98304,k=50,threads=4,weight='[aw=w]',vce='robust'),
]
def qr_input(spec,seed,warm=False):
 n=7680 if warm else spec['n'];k=spec['k'];path=BASE/'qr-public-data'/f"{spec['name']}-n{n}-s{seed}.dta"
 if path.exists():return path
 path.parent.mkdir(exist_ok=True)
 if spec.get('near_capacity'):
  r=np.arange(n);g=(r*((n*(k+1))//(2*(k+2)))//n).astype(float)
  x=[g/256+np.sin((g+1)*j*0.137)*(r%3-1) for j in range(1,k+1)];x[-1]=x[0].copy()
  y=g/16+np.sin(r*0.913)
  for j,col in enumerate(x):y=y+(j+1)/32*col
  frame=pd.DataFrame({'obs_id':r+1,'y':y,**{f'x{j+1}':col for j,col in enumerate(x)},'g1':g,'w':1+r%7,'c1':(r//120)%97,'c2':(r//20)%3})
  source_hash=sha256(__file__)
 else:
  base=input_for(spec,seed,warm);frame=pd.read_stata(base,convert_categoricals=False);frame[f'x{k}']=frame['x1'];source_hash=sha256(base)
 frame.to_stata(path,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 hashes={}
 for name in frame:
  values=frame[name].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[name]=hashlib.sha256(values.tobytes()).hexdigest()
 write_json(path.with_suffix('.json'),{'n':n,'k':k,'seed':seed,'columns':list(frame),'column_sha256':hashes,'source_sha256':source_hash,'sha256':sha256(path),'recipe':spec,'duplicate':f'x{k}=x1','near_capacity_seed_note':'Deterministic geometry; seed labels the invocation. No seed variation is claimed for near-capacity input.'})
 return path
if __name__=='__main__':
 ap=argparse.ArgumentParser();ap.add_argument('--name');ap.add_argument('--trace',action='store_true');ap.add_argument('--prepare',action='store_true');args=ap.parse_args();seed=67280421 if args.trace else 31415927
 specs=[s for s in CASES if not args.name or s['name']==args.name]
 if args.prepare:
  for s in specs:
   for warm in [False,True]:print(qr_input(s,seed+int(warm),warm),flush=True)
  raise SystemExit
 identities=read_json(BASE/'built-identities-v6.json');assert sha256(BASE/'protocol-v6.md')==identities['protocol_sha256']
 for i,spec in enumerate(specs):
  paths={};data,warm=qr_input(spec,seed),qr_input(spec,seed+1,True)
  for arm in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
   source=BASE/('candidate6' if arm=='candidate' else 'baseline');expected=identities[arm+'_plugin_sha256'];assert sha256(source/'dist/fereg_macos.plugin')==expected
   path=BASE/'runs'/f"qr6-{'profile-' if args.trace else ''}{spec['name']}-s{seed}-{arm}";paths[arm]=path
   manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
   result=measure(BASE,dict(model='ols',section='width',effects=['g1'],**spec),'fereg','warm',data,warm,path,manifest,identity={'base_commit':identities['baseline_commit'],'variant':arm,'seed':seed,'plugin_sha256':expected,'trace':args.trace},source=source,trace=args.trace)
   print(json.dumps({'case':spec['name'],'arm':arm,'status':result['status'],'seconds':result['seconds'],'native_bytes':result['metadata'].get('fereg_native_allocation_bytes'),'scratch_bytes':result['metadata'].get('fereg_scratch_bytes')}),flush=True)
   assert result['status']=='pass',str(path);write_json(path/'native-science.json',certify(path))
  comparison=baseline_compare(paths['candidate'],paths['baseline'])
  for name in ['fereg_status','fereg_dof','fereg_fe','fereg_cluster','fereg_subset']:
   a,b=(paths[arm]/(name+'.csv') for arm in ['candidate','baseline']);assert a.exists()==b.exists()
   if a.exists():assert a.read_bytes()==b.read_bytes(),(spec['name'],name)
  write_json(paths['candidate']/'structural-science.json',{'sample_status_rank_dof_weight_fe_cluster_exact':True,'numerical_comparison':comparison})
  b,c=(read_json(paths[a]/'result.json')['seconds'] for a in ['baseline','candidate'])
  review={'command_seconds_delta':c-b,'relative_change':c/b-1,'above_control_review_envelope':c-b>max(.010,.05*b)}
  write_json(paths['candidate']/'control-review.json',review);print(json.dumps({'case':spec['name'],'review':review}),flush=True)
