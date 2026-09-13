"""P3 isolated threshold experiment using existing local measurement/oracles."""
import argparse
import copy
import numpy as np
from common import sha256, write_json
from final_compare import baseline_compare, certify, matrix
from measure import measure
from preparation_compare import input_for, PRIMARY, CONTROLS
from run import BASE, MANIFEST

BASELINE='c37686b96b633db43eb1d7210f5ca3e7f9520e93'
TARGETS=[PRIMARY[0],PRIMARY[3],PRIMARY[2]]

def pair(spec,seed,rotation=0,trace=False,prefix=''):
 data,warm=input_for(spec,seed),input_for(spec,seed+1,warm=True)
 cell=dict(model='ols',section='width',**spec)
 cell.setdefault('effects',['g1'] if spec['design']=='one_fe' else ['g1','g2'])
 paths={}
 for arm in (['candidate','baseline'] if rotation%2 else ['baseline','candidate']):
  source=BASE/arm;path=BASE/'runs'/f'inference-{prefix}{spec["name"]}-s{seed}-{arm}'
  paths[arm]=path
  manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
  result=measure(BASE,cell,'fereg','warm',data,warm,path,manifest,
   identity=dict(base_commit=BASELINE,variant=arm,seed=seed,trace=trace,plugin_sha256=sha256(source/'dist/fereg_macos.plugin')),source=source,trace=trace)
  print(path.name,result['status'],result['seconds'],flush=True)
  assert result['status']=='pass'
  write_json(path/'native-science.json',certify(path))
  if trace:write_json(path/'phases.json',[l for l in (path/'stderr.txt').read_text().splitlines() if l.startswith('FEREG_PHASE|')])
 a,b=paths['candidate'],paths['baseline']
 science=baseline_compare(a,b)
 for name in ['fereg_status','fereg_dof','fereg_fe','fereg_cluster','fereg_subset']:
  x,y=a/(name+'.csv'),b/(name+'.csv')
  assert x.exists()==y.exists()
  if x.exists():assert x.read_bytes()==y.read_bytes(),name
 # Columns 5–8 are numerical certificate metrics; they are independently
 # checked under the original tolerance, rather than mistaken for decisions.
 cols=[0,1,2,3,10,11,12,13,14,15]
 assert np.array_equal(matrix(a,'fereg_solver')[:,cols],matrix(b,'fereg_solver')[:,cols])
 if (a/'saved_outputs.csv').exists():
  import pandas as pd
  x,y=(pd.read_csv(p/'saved_outputs.csv') for p in [a,b])
  assert list(x)==list(y)
  assert np.allclose(x.to_numpy(float),y.to_numpy(float),atol=1e-10,rtol=1e-7,equal_nan=True)
 science['solver_decisions_exact']=True
 write_json(a/'inference-science.json',science)
 return paths

