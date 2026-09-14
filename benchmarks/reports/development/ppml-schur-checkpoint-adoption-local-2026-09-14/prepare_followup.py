import os
from pathlib import Path
import json,copy,hashlib,datetime
import numpy as np,pandas as pd
from ppml_preparation_compare import input_for
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();requests=[]
def bound(name,cell,data,warm,kind,seed):
 x={'name':name,'cell':cell,'kind':kind,'seed':seed,'data':str(data),'warm':str(warm)}
 for key,p in [('data',data),('warm',warm)]:x[key+'_sha256']=sha(p);x[key+'_metadata_sha256']=sha(p.with_suffix('.json'))
 requests.append(x)
for req in json.loads((r/'requests.json').read_text()):
 for seed in [8675309,20260819]:
  cell=req['cell'];bound(req['name']+'-s'+str(seed),cell,input_for(cell,seed),input_for(cell,seed+1,warm=True),'repeat',seed)
for design in ['weak_d3','bottleneck_ring','cycle','disconnected_blocks','unbalanced','three_fe_sparse']:
 cell=dict(name=design+'_wide',design=design,n=30720,k=17,threads=4,model='ppml',section='width',effects=['g1','g2','g3'] if design=='three_fe_sparse' else ['g1','g2'],vce='robust')
 bound(cell['name'],cell,input_for(cell,31415927),input_for(cell,31415928,warm=True),'control',31415927)
for kind in ['weighted_cut','rank_deficient']:
 cell=dict(name=kind,design='two_fe_dense',n=16384 if kind=='weighted_cut' else 30720,k=17,threads=4,model='ppml',section='width',effects=['g1','g2'],vce='robust')
 if kind=='weighted_cut':cell['weight']='[pw=w]'
 paths=[]
 for warm,seed in [(False,31415927),(True,31415928)]:
  source=input_for(cell,seed,warm=warm);frame=pd.read_stata(source,convert_categoricals=False);i=np.arange(len(frame))
  if kind=='weighted_cut':
   frame.g1=i//4+1;base=i//4%128;frame.g2=np.where(i%4==0,base,np.where(i%4==1,(base+1)%128,(i*37+i//512*19)%128))+1
   frame.w=np.where(i%4<2,1.0,1e-10)
  else:frame.x17=2*frame.x1
  out=r/'derived-data'/source.name;out.parent.mkdir(exist_ok=True);assert not out.exists()
  frame.to_stata(out,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
  info=json.loads(source.with_suffix('.json').read_text());hashes={}
  for c in frame:
   values=frame[c].to_numpy(dtype='<f8',copy=True);values[np.isnan(values)]=float.fromhex('0x1p+1023');hashes[c]=hashlib.sha256(values.tobytes()).hexdigest()
  info.update(schema='ppml-schur-checkpoint-protected-v1',source_sha256=sha(source),sha256=sha(out),columns=list(frame),column_sha256=hashes,derivation=kind)
  out.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n');paths.append(out)
 bound(kind,cell,*paths,'control',31415927)
old=json.loads(Path(os.path.join(os.environ['PREDICTOR_ROOT'], 'followup-requests.json')).read_text())
for name in ['small_narrow','low_fw','slope_numeric']:
 x=next(q for q in old if q['name']==name and q.get('kind')=='control');bound(name,x['cell'],Path(x['data']),Path(x['warm']),'control',31415927)
(r/'followup-requests.json').write_text(json.dumps(requests,indent=2)+'\n');print('FOLLOWUP_INPUTS_TERMINAL_PASS',len(requests),flush=True)
