from pathlib import Path
import numpy as np,pandas as pd,hashlib,json,datetime
root=Path(__file__).resolve().parents[1];bindings={}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
for n in [16384,32768]:
 name=f'singleton_cascade_{n}';j=np.arange(n);block=j//16;slot=j%16
 # Eight retained square rows; an eight-edge tail is deleted recursively.
 a=np.array([0,0,1,1,0,0,1,1,0,2,2,3,3,4,4,5]);b=np.array([0,1,0,1,0,1,0,1,2,2,3,3,4,4,5,5])
 x1=np.sin((j+1)*0.713);x2=np.cos((j+1)*0.317)
 data={'obs_id':j+1,'y':1+.7*x1-.2*x2+np.sin((j+1)*1.113)*.13+block*.0001,'x1':x1,'x2':x2,'g1':block*6+a[slot]+1,'g2':block*6+b[slot]+1,'tag':np.ones(n),'w':1+(j%5)*.25}
 frame=pd.DataFrame({k:np.asarray(v,dtype='<f8') for k,v in data.items()});p=root/'data'/(name+'.dta')
 frame.to_stata(p,write_index=False,version=118,time_stamp=datetime.datetime(2026,9,16))
 meta={'n':n,'columns':list(frame.columns),'column_sha256':{k:hashlib.sha256(frame[k].to_numpy(dtype='<f8').tobytes()).hexdigest() for k in frame.columns}}
 p.with_suffix('.json').write_text(json.dumps(meta,indent=2)+'\n')
 bindings[name]={'data_sha256':sha(p),'metadata_sha256':sha(p.with_suffix('.json')),'expected_sample_rows':n//2}
(root/'generated-bindings.json').write_text(json.dumps(bindings,indent=2)+'\n')
