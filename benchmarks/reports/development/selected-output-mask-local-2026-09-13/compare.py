import argparse,copy,json
from common import read_json,write_json,sha256
from run import BASE,MANIFEST
from preparation_compare import input_for
from final_compare import baseline_compare,certify
from measure import measure
PRIMARY=[
 dict(name='unit_saved',design='one_fe',n=983040,k=1,threads=4,absorb='saved_g=g1'),
 dict(name='aw_low_saved',design='one_fe',n=983040,k=1,threads=4,weight='[aw=w]',absorb='saved_g=g1',fit_options='memory(low)',missing=True),
 dict(name='pw_two_fe_partial',design='strong_d2',n=245760,k=10,threads=16,weight='[pw=w]',vce='cluster c1',absorb='saved_g=g1 g2',missing=True),
]
CONTROLS=[
 dict(name='small_saved',design='one_fe',n=7680,k=1,threads=1,weight='[aw=w]',absorb='saved_g=g1'),
 dict(name='no_outputs',design='one_fe',n=245760,k=50,threads=16),
 dict(name='weak_saved',design='weak_d3',n=30720,k=10,threads=4,absorb='saved_g=g1 g2'),
]
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--prepare',action='store_true');p.add_argument('--controls',action='store_true');p.add_argument('--trace',action='store_true');p.add_argument('--name');p.add_argument('--seed',type=int,default=104729);args=p.parse_args();specs=CONTROLS if args.controls else PRIMARY;specs=[s for s in specs if not args.name or s['name']==args.name];seed=67280421 if args.trace else args.seed
 if args.prepare:
  for spec in specs:
   for warm in [False,True]:print(input_for(spec,seed+int(warm),warm),flush=True)
  raise SystemExit
 ids=read_json(BASE/'built-identities.json');assert sha256(BASE/'protocol.md')==ids['protocol_sha256'];assert sha256(__file__)==ids['harness_sha256']
 for i,spec in enumerate(specs):
  paths={};data,warm=input_for(spec,seed),input_for(spec,seed+1,True);cell=dict(model='ols',section='width',effects=['g1'] if spec['design']=='one_fe' else ['g1','g2'],**spec)
  for arm in (['baseline','candidate'] if i%2==0 else ['candidate','baseline']):
   source=BASE/arm;assert sha256(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin_sha256'];path=BASE/'runs'/f"{'profile' if args.trace else 'screen'}-{spec['name']}-s{seed}-{arm}";paths[arm]=path
   manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
   result=measure(BASE,cell,'fereg','warm',data,warm,path,manifest,source=source,trace=args.trace,identity={'variant':arm,'plugin_sha256':ids[arm+'_plugin_sha256'],'baseline_commit':ids['baseline_commit'],'seed':seed})
   print(json.dumps({'case':spec['name'],'arm':arm,'status':result['status'],'seconds':result['seconds'],'native_bytes':result['metadata'].get('fereg_native_allocation_bytes')}),flush=True);assert result['status']=='pass',str(path);write_json(path/'native-science.json',certify(path))
  comparison=baseline_compare(paths['candidate'],paths['baseline'])
  for name in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv','saved_outputs.csv']:
   a,b=(paths[x]/name for x in ['baseline','candidate']);assert a.exists()==b.exists()
   if a.exists():assert a.read_bytes()==b.read_bytes(),(spec['name'],name)
  write_json(paths['candidate']/'science.json',{'comparison':comparison,'sample_rank_status_dof_fe_cluster_saved_exact':True})
  b,c=(read_json(paths[x]/'result.json') for x in ['baseline','candidate']);gain=b['seconds']-c['seconds'];mb=b['metadata']['fereg_native_allocation_bytes'];mc=c['metadata']['fereg_native_allocation_bytes'];memory=mb-mc
  decision={'command_delta':-gain,'command_change_percent':-100*gain/b['seconds'],'native_bytes_delta':-memory,'native_change_percent':-100*memory/mb,'useful_screen':(gain>=.010 and gain/b['seconds']>=.05) or (memory>=2*1024**2 and memory/mb>=.05 and c['seconds']<=1.05*b['seconds']),'above_review_envelope':-gain>max(.010,.05*b['seconds'])};write_json(paths['candidate']/'screen-decision.json',decision);print(json.dumps({'case':spec['name'],**decision}),flush=True)
