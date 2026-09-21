import os,sys,json,hashlib,shutil,datetime,importlib.util
from pathlib import Path
import numpy as np,pandas as pd
ROOT=Path(os.environ['FEREG_SLOPE_ROOT'])
REPO=Path(__file__).resolve().parents[4]
PROFILE=REPO/'target/one-fe-profile-20260919'
SOURCE=REPO/'target/one-fe-optimization-20260919/residualoverlap'
sys.path.insert(0,str(ROOT/'controller'))
from common import read_json,write_json,sha256
from measure import measure
spec=importlib.util.spec_from_file_location('generate',REPO/'benchmarks/systematic/generate.py');gen=importlib.util.module_from_spec(spec);spec.loader.exec_module(gen)
manifest=read_json(REPO/'benchmarks/systematic/manifest.json')
manifest['measurement']['command_timeout_seconds']=120
manifest['measurement']['rss_interval_seconds']=.05
(ROOT/'work/refs').mkdir(parents=True,exist_ok=True)
assert os.environ.get('STATA_MP_PATH')
os.environ['R_LIBS_USER']=str(PROFILE/'R-stock')+':' + os.environ['FEREG_SLOPE_R_DEPS']
r=ROOT/'controller/r_worker.R';s=r.read_text();s=s.replace('paste(cell$effects, collapse=" + ")','if (isTRUE(cell$absorbed_slope)) "g1[z]" else paste(cell$effects, collapse=" + ")');s=s.replace('if (isTRUE(c$trace)) Rprof(file.path(output,"Rprof.out"), interval=.001)','');s=s.replace('if (isTRUE(c$trace)) {\n Rprof(NULL)\n write.csv(summaryRprof(file.path(output,"Rprof.out"))$by.self, file.path(output,"Rprof-summary.csv"))\n}','');r.write_text(s)
s=ROOT/'controller/stata_worker.do';s.write_text(s.read_text().replace('if "`trace\'" == "1" profiler on','').replace('if "`trace\'" == "1" profiler off','').replace('if "`trace\'" == "1" profiler report',''))

def data(n,k):
 p=ROOT/'data'/f'n{n}-k{k}.dta'
 if p.exists():return p
 c=dict(model='ols',design='one_fe',n=n,k=k,threads=1,effects=['g1'],section='width')
 gen.generate(c,104729,p,manifest)
 f=pd.read_stata(p,convert_categoricals=False)
 rng=np.random.default_rng(719)
 f['z']=rng.normal(size=n)
 f['y']+=.3*np.sin(f.g1)*f.z
 f.to_stata(p,write_index=False,version=118,time_stamp=datetime.datetime(2000,1,1))
 info=read_json(p.with_suffix('.json'));info['columns']=list(f);info['column_sha256']={c:hashlib.sha256(np.asarray(f[c],dtype='<f8').tobytes()).hexdigest() for c in f};info['sha256']=sha256(p);write_json(p.with_suffix('.json'),info)
 with p.with_suffix('.bin').open('wb') as h:
  for c in f:h.write(np.asarray(f[c],dtype='<f8').tobytes())
 return p

def run(n,k,t,slope,arm,rep,solver='auto'):
 p=data(n,k);warm=data(7680 if n>7680 else n,k)
 cell=dict(model='ols',design='one_fe',n=n,k=k,threads=t,effects=['g1'],section='width',absorbed_slope=slope,absorb='g1##c.z' if slope else 'g1')
 output=ROOT/'runs'/f'n{n}-k{k}-t{t}-s{int(slope)}-{arm}-{solver}-r{rep}'
 if output.exists():return output
 result=measure(ROOT,cell,arm,'warm',p,warm,output,manifest,source=SOURCE,trace=True,solver=solver)
 print(output.name,result['status'],result['seconds'],flush=True)
 assert result['status']=='pass',(output,result)
 return output
write_json(ROOT/'identity.json',dict(source_artifact=str(SOURCE),plugin_sha256=sha256(SOURCE/'dist/fereg_macos.plugin'),fixest_sha256=sha256(PROFILE/'fixest.tar.gz')))
if sys.argv[-1]=='smoke':
 for a in ['fereg','fixest']:run(360,2,1,True,a,0)
else:
 for k in [10,50]:
  for t in [1,16]:
   for rep in range(3):
    for arm in (['fereg','fixest'] if rep%2==0 else ['fixest','fereg']):run(245760,k,t,True,arm,rep)
  for arm in ['fereg','fixest']:run(245760,k,16,False,arm,0)
  run(245760,k,16,True,'fereg',0,'map')
