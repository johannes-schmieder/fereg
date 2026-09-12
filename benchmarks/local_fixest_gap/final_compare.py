"""Bounded sequential local development comparison; never submits cluster work."""
import copy, json, os
from pathlib import Path
import numpy as np
import pandas as pd
from common import read_json, write_json, sha256
from run import BASE, MANIFEST, data_for, compare
from measure import measure


def matrix(path,name):
 return pd.read_csv(path/(name+'.csv')).pivot(index='row',columns='column',values='value').to_numpy()

def certify(path):
 d=read_json(path/'result.json'); assert d['status']=='pass'
 c=matrix(path,'fereg_certificate')
 for row in [1,2,3,24,32]: assert c[row-1,0]==c[row-1,1]==1,(path,row)
 sol=matrix(path,'fereg_solver');assert np.all(sol[:,3]==2)
 assert np.all(sol[:,5]<=d['metadata']['fereg_tolerance'])
 assert d['metadata']['converged']==1
 return {'native_rank_score_bread_covariance_determinant':True,'initial_and_final_FE':True,'max_FE_relative_infinity':float(sol[:,5].max())}

def baseline_compare(a,b):
 out=compare(a,b,"baseline")
 ra,rb=read_json(a/'result.json'),read_json(b/'result.json')
 for k in ['N','df_r','rank','df_m','df_a','df_a_initial','df_a_redundant','df_a_nested','N_full','num_singletons','sumweights','report_constant','N_hdfe']:
  assert ra['metadata'][k]==rb['metadata'][k],(k,ra['metadata'][k],rb['metadata'][k])
 assert np.array_equal(matrix(a,'fereg_status'),matrix(b,'fereg_status'))
 for name in ['b','V']:
  x,y=matrix(a,name),matrix(b,name);assert x.shape==y.shape
  assert np.allclose(x,y,atol=1e-10,rtol=1e-6 if name=='V' else 1e-7)
  out['full_'+name+'_maxabs']=float(np.max(np.abs(x-y)))
 out.update({'full_b_V_including_constant':True,'sample_status_rank_DoF_exact':True})
 write_json(a/'baseline-comparison.json',out)
 return out


def invoke(label,design,n,k,threads,seed,arm,source,trace=False,cap=90):
 data,cell=data_for(design,n,k,seed);warm,_=data_for(design,7680,k,seed+1);cell['threads']=threads
 p=BASE/'runs'/f'{label}-{design}-n{n}-k{k}-t{threads}-s{seed}-{arm}'
 spec=copy.deepcopy(MANIFEST);spec['measurement']['command_timeout_seconds']=cap
 identity={'candidate_base_commit':MANIFEST['candidate_commit'],'variant':source.name,'plugin_sha256':sha256(source/'dist/fereg_macos.plugin') if arm=='fereg' else None,
           'fixest_version':'0.14.2','trace':trace,'solver':'auto','seed':seed,'poolsize':int(os.environ.get('FEREG_LOCAL_POOL','10'))}
 print('RUN',p.name,flush=True)
 r=measure(BASE,cell,arm,'warm',data,warm,p,spec,identity=identity,source=source,trace=trace)
 print('RESULT',r['status'],r['seconds'],flush=True)
 if arm=='fereg' and r['status']=='pass':write_json(p/'native-science.json',certify(p))
 return p

if __name__=='__main__':
 import argparse
 ap=argparse.ArgumentParser();ap.add_argument('--part',choices=['primary','holdout','overhead','oracle','pool'],required=True);a=ap.parse_args()
 base=BASE/'baseline';proto=BASE/'schur-prototype'
 if a.part=='primary':
  for threads in [4,16]:
   for design in ['one_fe','strong_d3','strong_d6']:
    for rep,seed in enumerate([104729,8675309,20260819]):
     arms=[('baseline','fereg',base),('prototype','fereg',proto),('fixest','fixest',base)] if design!='one_fe' else [('baseline','fereg',base),('fixest','fixest',base)]
     arms=arms[rep%len(arms):]+arms[:rep%len(arms)]
     paths={name:invoke('repeat-'+name,design,983040,50,threads,seed,arm,source) for name,arm,source in arms}
     for name in ['baseline','prototype']:
      if name in paths:print('SCIENCE',name,compare(paths[name],paths['fixest']),flush=True)
     if 'prototype' in paths:print('BASELINE',baseline_compare(paths['prototype'],paths['baseline']),flush=True)
 elif a.part=='overhead':
  for n in [7680,983040]:
   for k in [1,10]:
    paths=[invoke('overhead', 'one_fe',n,k,4,31415927,arm,base,trace=True) for arm in ['fereg','fixest']]
    print('SCIENCE',compare(*paths),flush=True)
 elif a.part=='holdout':
  for design,n in [('two_fe_dense',245760),('weak_d3',61440)]:
   paths={name:invoke('holdout-'+name,design,n,50,4,31415927,arm,source,cap=60) for name,arm,source in [('baseline','fereg',base),('prototype','fereg',proto),('fixest','fixest',base)]}
   for name in ['baseline','prototype']:
    if read_json(paths[name]/'result.json')['status']=='pass':print('SCIENCE',name,compare(paths[name],paths['fixest']),flush=True)
   if all(read_json(paths[x]/'result.json')['status']=='pass' for x in ['baseline','prototype']):print('BASELINE',baseline_compare(paths['prototype'],paths['baseline']),flush=True)
 elif a.part=='pool':
  for threads in [4,16]:
   os.environ['FEREG_LOCAL_POOL']='16' if threads==16 else '8'
   for design in ['strong_d3','strong_d6']:
    p=invoke('pool'+os.environ['FEREG_LOCAL_POOL'],design,983040,50,threads,104729,'fereg',base,trace=True)
    expected=BASE/'runs'/f'repeat-baseline-{design}-n983040-k50-t{threads}-s104729-fereg'
    print('BASELINE',baseline_compare(p,expected),flush=True)
 else:
  from oracle import oracle
  for design in ['one_fe','two_fe_dense','strong_d3','strong_d6','weak_d3']:
   seed=67280421;n=7680;k=50;threads=4
   p=invoke('oracle-prototype',design,n,k,threads,seed,'fereg',proto)
   data,cell=data_for(design,n,k,seed);o=oracle(data,cell);write_json(p/'independent-oracle.json',o)
   b=pd.read_csv(p/'coefficients.csv').set_index('name').loc[o['coefficient_names'],'value'].to_numpy()
   V=pd.read_csv(p/'covariance.csv').pivot(index='row',columns='column',values='value').loc[o['coefficient_names'],o['coefficient_names']].to_numpy()
   assert np.allclose(b,o['coefficients'],atol=1e-10,rtol=1e-7)
   assert np.allclose(V,o['covariance'],atol=1e-10,rtol=1e-6)
   meta=read_json(p/'result.json')['metadata'];assert meta['df_r']==o['df_r']
   assert np.isclose(meta['rss'],o['rss'],atol=1e-10,rtol=1e-8)
   write_json(p/'oracle-comparison.json',{'status':'pass','coefficients_maxabs':float(np.max(np.abs(b-o['coefficients']))),'V_maxabs':float(np.max(np.abs(V-o['covariance']))),'df_r_exact':True,'rss_relative_error':abs(meta['rss']/o['rss']-1)})
   print('ORACLE PASS',design,flush=True)
