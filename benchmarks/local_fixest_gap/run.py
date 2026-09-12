import argparse, csv, hashlib, json, os, sys
from pathlib import Path
import numpy as np
import pandas as pd
from common import read_json,write_json,sha256
sys.path.append(str(Path(__file__).resolve().parents[1] / "systematic"))
from generate import generate
from measure import measure
BASE=Path(os.environ.get('FEREG_LOCAL_BENCH_ROOT', Path(__file__).resolve().parents[2] / 'target/local-fixest-gap'))
MANIFEST=read_json(Path(__file__).resolve().parents[1] / 'systematic/manifest.json')

def data_for(design,n,k,seed):
 cell={'model':'ols','design':design,'n':n,'k':k,'threads':4,'section':'width','effects':MANIFEST['designs'][design]['effects']}
 p=BASE/'data'/f'{design}-n{n}-k{k}-s{seed}.dta'
 if not p.exists():
  generate(cell,seed,p,MANIFEST)
  frame=pd.read_stata(p,convert_categoricals=False)
  with p.with_suffix('.bin').open('wb') as f:
   for c in frame:f.write(np.asarray(frame[c],dtype='<f8').tobytes())
 return p,cell

def compare(a,b,label="fixest"):
 pa,pb=Path(a),Path(b)
 ra,rb=read_json(pa/'result.json'),read_json(pb/'result.json')
 assert ra['status']==rb['status']=='pass',(ra['status'],rb['status'])
 assert ra['sample_sha256']==rb['sample_sha256']
 assert ra['metadata']['N']==rb['metadata']['N']
 diffs={}
 for file,keys,cols,rtol in [('coefficients.csv',['name'],['value','se'],1e-7),('covariance.csv',['row','column'],['value'],1e-6)]:
  xa=pd.read_csv(pa/file).set_index(keys).sort_index();xb=pd.read_csv(pb/file).set_index(keys).sort_index()
  assert xa.index.equals(xb.index)
  for col in cols:
   u,v=xa[col].to_numpy(),xb[col].to_numpy()
   diffs[file+':'+col]=float(np.max(np.abs(u-v)))
   assert np.all(np.isfinite(u)) and np.all(np.isfinite(v))
   assert np.allclose(u,v,atol=1e-10,rtol=1e-6 if col=='se' else rtol),(file,col,diffs)
 assert np.isclose(ra['metadata']['rss'],rb['metadata']['rss'],atol=1e-10,rtol=1e-8)
 diffs['sample_exact']=True
 diffs['reference_arm'] = rb['arm']
 write_json(pa/(label+'-comparison.json'),diffs)
 return diffs

if __name__ == '__main__':
 p=argparse.ArgumentParser();p.add_argument('--label',required=True);p.add_argument('--source',type=Path,default=BASE/'baseline');p.add_argument('--n',type=int,default=245760);p.add_argument('--k',type=int,default=50);p.add_argument('--threads',type=int,default=4);p.add_argument('--designs',nargs='+',default=['one_fe','two_fe_dense','strong_d3','strong_d6']);p.add_argument('--arms',nargs='+',default=['fereg','fixest']);p.add_argument('--trace',action='store_true');p.add_argument('--solver',default='auto');p.add_argument('--seed',type=int,default=104729)
 a=p.parse_args()
 for design in a.designs:
  data,cell=data_for(design,a.n,a.k,a.seed);warm,_=data_for(design,7680,a.k,a.seed+1);cell['threads']=a.threads
  outputs={}
  for arm in a.arms:
   output=BASE/'runs'/f'{a.label}-{design}-n{a.n}-k{a.k}-t{a.threads}-{arm}'
   outputs[arm]=output
   print('RUN',output.name,flush=True)
   receipt=measure(BASE,cell,arm,'warm',data,warm,output,MANIFEST,source=a.source,trace=a.trace,solver=a.solver)
   print('RESULT',receipt['status'],receipt['seconds'],round(receipt['peak_rss_gib'] or 0,3),flush=True)
   if receipt['status']!='pass':raise RuntimeError(output)
   text='\n'.join(f.read_text(errors='replace') for f in [output/'stderr.txt',*output.glob('*.log')])
   phase=[line for line in text.splitlines() if line.startswith('FEREG_PHASE|')]
   if phase:print('\n'.join(phase[-2:]),flush=True)
  if set(outputs)=={'fereg','fixest'}: print('SCIENCE',compare(outputs['fereg'],outputs['fixest']),flush=True)
