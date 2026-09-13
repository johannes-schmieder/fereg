from pathlib import Path
import argparse,copy,json
import numpy as np
from common import read_json,write_json,sha256
from run import BASE,MANIFEST
from measure import measure
from preparation_compare import input_for as ols_input
from weighted_schur_compare import input_for as weighted_input
from ppml_preparation_compare import input_for as ppml_input,compare as ppml_compare
from final_compare import baseline_compare,certify,matrix
OLD=Path('PREVIOUS_EXPERIMENT')
CONTROLS=[dict(name=d,design=d,n=30720,k=16,threads=4,vce='robust') for d in ['weak_d3','two_fe_dense','bottleneck_ring','cycle','disconnected_blocks','unbalanced']]+[
 dict(name='small_aw',design='one_fe',n=1920,k=2,threads=1,weight='[aw=w]',vce='robust'),
 dict(name='low_fw',design='strong_d3',n=30720,k=16,threads=4,weight='[fw=w]',vce='cluster c1',fit_options='memory(low)'),
 dict(name='unit_saved',design='strong_d2',n=245760,k=50,threads=16,absorb='saved_g=g1 saved_h=g2',fit_options='residuals(saved_e)'),
 dict(name='weighted_cut',design='strong_d3',n=30720,k=16,threads=4,weight='[aw=w]',weight_frame='cut'),
 dict(name='complete_iid',design='one_fe',n=983040,k=50,threads=16),
 dict(name='ppml_weak_pw',design='weak_d3',n=7680,k=16,threads=4,weight='[pw=w]',vce='cluster c1',model='ppml')]
def inputs(spec,seed):
 if spec['name']=='unit_saved' and seed==104729:
  req=read_json(OLD/'runs/screen-unit_saved-s104729-baseline/request.json');return Path(req['data']),Path(req['warm'])
 fn=ppml_input if spec.get('model')=='ppml' else weighted_input if spec.get('weight_frame') else ols_input
 return fn(spec,seed),fn(spec,seed+1,True)
def invoke(cell,data,warm,arm,label,trace=False):
 ids=read_json(BASE/'built-identities.json');source=BASE/arm
 assert sha256(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin_sha256']
 assert sha256(BASE/'protocol.md')==ids['protocol_sha256'] and sha256(__file__)==ids['harness_sha256']
 path=BASE/'runs'/f'{label}-{arm}'
 m=copy.deepcopy(MANIFEST);m['measurement']['command_timeout_seconds']=180
 result=measure(BASE,cell,'feppml' if cell.get('model')=='ppml' else 'fereg','warm',data,warm,path,m,source=source,trace=trace,identity={'variant':arm,'plugin_sha256':ids[arm+'_plugin_sha256'],'baseline_commit':ids['baseline_commit'],'purpose':label})
 print(json.dumps({'run':path.name,'status':result['status'],'seconds':result['seconds'],'native_bytes':result['metadata'].get('fereg_native_allocation_bytes')}),flush=True)
 assert result['status']=='pass',str(path)
 if cell.get('model')!='ppml':write_json(path/'native-science.json',certify(path))
 return path

def replay(trace=False):
 req=read_json(OLD/'runs/screen-pw_partial-s104729-baseline/request.json')
 assert sha256(Path(req['data']))=='eecf8666668f96734ee06c52310b9920f29c65fc9cce86fd3707be22399fc7d0'
 path=invoke(req['cell'],Path(req['data']),Path(req['warm']),'candidate','original-pw-trace' if trace else 'original-pw-replay',trace)
 assert (path/'saved_outputs.csv').is_file()
 if trace:baseline_compare(path,BASE/'runs/original-pw-replay-candidate')

def pair(s,seed,rotation,label):
 data,warm=inputs(s,seed);cell=dict(model='ols',section='width');cell.update(s)
 cell['effects']=['g1'] if s['design']=='one_fe' else ['g1','g2']
 paths={arm:invoke(cell,data,warm,arm,label) for arm in (['candidate','baseline'] if rotation%2 else ['baseline','candidate'])}
 if cell['model']=='ppml':ppml_compare(paths)
 else:
  baseline_compare(paths['candidate'],paths['baseline'])
  names=['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset','saved_outputs']
  for name in names:
   a,b=(paths[x]/f'{name}.csv' for x in ['baseline','candidate']);assert a.exists()==b.exists(),name
   if a.exists():assert a.read_bytes()==b.read_bytes(),(label,name)
  cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15]
  assert np.array_equal(matrix(paths['candidate'],'fereg_solver')[:,cols],matrix(paths['baseline'],'fereg_solver')[:,cols])
 a,b=(read_json(paths[x]/'result.json') for x in ['baseline','candidate']);delta=b['seconds']-a['seconds']
 write_json(paths['candidate']/'cost-review.json',dict(science_exact=True,seconds_change=delta,percent_change=100*delta/a['seconds'],review=delta>max(.010,.05*a['seconds'])))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('part',choices=['prepare','replay','trace','controls','recheck']);p.add_argument('--name');a=p.parse_args()
 if a.part in ['replay','trace']:replay(a.part=='trace')
 elif a.part=='recheck':
  s=next(s for s in CONTROLS if s['name']==a.name)
  for i,seed in enumerate([8675309,20260819,67280421]):pair(s,seed,i+1,f'recheck-{s["name"]}-s{seed}')
 else:
  for i,s in enumerate(CONTROLS):
   seed=104729 if s['name']=='unit_saved' else 31415927
   if a.part=='prepare':inputs(s,seed);print('prepared',s['name'],flush=True)
   else:pair(s,seed,i,f'control-{s["name"]}-s{seed}')
