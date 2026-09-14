import os
from pathlib import Path
import os,json,hashlib,copy,re
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
import numpy as np
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text());requests=json.loads((root/'screen-requests.json').read_text());results=[]
for kind,order,names,trace in [('repeat-1',['candidate','baseline'],['low_weak'],False),('repeat-2',['baseline','candidate'],['low_weak'],False),('profile',['baseline','candidate'],['low_weak'],True),('wide-profile',['candidate'],['low_aw_robust'],True)]:
 for name in names:
  req=next(x for x in requests if x['name']==name);os.environ['FEREG_LOCAL_POOL']=str(req['cell'].get('pool',10));os.environ['FEREG_LOCAL_BLOCK_WIDTH']=req.get('block_width','1');row={'name':name,'kind':kind,'diagnostic':trace};paths={}
  for arm in order:
   assert sha(root/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin'];assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
   out=root/'runs'/(kind+'-'+name+'-'+arm);settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
   result=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,settings,source=root/arm,trace=trace,identity={'artifact':ids[arm+'_plugin'],'protocol':sha(root/'followup-protocol.md'),'kind':kind,'harness':sha(Path(__file__))})
   assert result['status']=='pass',out;certify(out);row[arm]=result;paths[arm]=out
   row[arm+'_phase_lines']=[z for z in (out/'stderr.txt').read_text().splitlines() if z.startswith('FEREG_PHASE')]
   if trace:
    ref=root/'runs'/('screen-'+name+'-'+arm)
    for filename in ['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:
     assert (out/filename).read_bytes()==(ref/filename).read_bytes(),(name,filename)
   print(kind,name,arm,result['seconds'],flush=True)
  if 'baseline' in paths:
   row['comparison']=baseline_compare(paths['candidate'],paths['baseline'])
   for filename in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:assert (paths['candidate']/filename).read_bytes()==(paths['baseline']/filename).read_bytes(),(name,filename)
   cols=[0,1,2,3,10,11,12,13,14,15];assert np.array_equal(matrix(paths['candidate'],'fereg_solver')[:,cols],matrix(paths['baseline'],'fereg_solver')[:,cols])
   b,c=row['baseline']['seconds'],row['candidate']['seconds'];row.update(delta=c-b,percent=100*(c-b)/b)
  results.append(row);(root/'repeat-profile-results.json').write_text(json.dumps(results,indent=2)+'\n')
print('LOW_SCORE_REPEAT_PROFILE_TERMINAL_PASS',flush=True)
