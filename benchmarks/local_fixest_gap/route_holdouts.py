"""Additional local routing holdouts; never changes the frozen generator."""
import numpy as np, pandas as pd, hashlib, datetime
from run import data_for, BASE
from common import write_json, sha256

def custom(design,n,k,seed):
 source,_=data_for('strong_d3',n,k,seed)
 p=BASE/'data'/f'{design}-n{n}-k{k}-s{seed}.dta'
 if p.exists():return p
 frame=pd.read_stata(source,convert_categoricals=False);i=np.arange(n);worker=i//3;period=i%3;firms=n//120;block=16
 base=worker%firms;part=base//block;layer=worker//firms
 offset=np.where(period==1,1+layer%15,np.where(period==2,5+7*layer,0));firm=part*block+(base+offset)%block
 if design=='bottleneck_ring':
  bridge=(layer==0)&(base%block==0)&(period==2);firm[bridge]=(base[bridge]+block)%firms
 elif design=='unbalanced':
  remap=np.arange(firms);remap[firms//2:]=np.arange(firms//2)//4
  firm=remap[frame.g2.to_numpy(dtype=np.int64)-1]
 elif design=='cycle':
  firm=np.where(period==1,(base+1)%firms,base)
 elif design!='disconnected_blocks':raise ValueError(design)
 frame.g2=firm+1
 # A low-frequency FE component makes the bottleneck direction relevant to y.
 frame.y += np.sin(2*np.pi*firm/firms)
 frame.to_stata(p,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 with p.with_suffix('.bin').open('wb') as f:
  for c in frame:f.write(np.asarray(frame[c],dtype='<f8').tobytes())
 info={'schema':'solver-local-holdout-v1','n':n,'k':k,'seed':seed,'design':design,'model':'ols','section':'width','sha256':sha256(p),'columns':list(frame),'column_sha256':{c:hashlib.sha256(np.asarray(frame[c],dtype='<f8').tobytes()).hexdigest() for c in frame}}
 write_json(p.with_suffix('.json'),info);return p
