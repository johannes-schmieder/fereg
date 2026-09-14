import os
from pathlib import Path
import os,json,hashlib,copy
import numpy as np,pandas as pd
from measure import measure
from cap_measure import measure as cap_measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());rows=[]
for arm,key in [('baseline','inputs'),('candidate','candidate_inputs')]:
 assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin'];assert all(sha(r/arm/p)==h for p,h in ids[key].items())
weak=next(x for x in json.loads((r/'followup-requests.json').read_text()) if x['name']=='low_weak');rank=json.loads((r/'rank-robust-request.json').read_text());caps=json.loads((r/'low-admission-replay.json').read_text())['usable_caps']
for kind,req,cap,trace,arms in [('rank-robust',rank,None,False,['baseline','candidate']),('rank-profile',rank,None,True,['candidate']),('cap-lsmr',weak,caps['lsmr'],True,['baseline','candidate']),('cap-cg',weak,caps['cg'],True,['candidate','baseline'])]:
 row={'kind':kind,'name':req['name'],'diagnostic':trace,'usable_cap':cap};paths={}
 os.environ['FEREG_LOCAL_POOL']=str(req['cell'].get('pool',10));os.environ['FEREG_LOCAL_BLOCK_WIDTH']=req.get('block_width','1')
 assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for arm in arms:
  if cap is not None:os.environ['FEREG_MEMORY_LIMIT_BYTES']=str(cap+536870912)
  out=r/'runs'/(kind+'-'+req['name']+'-'+arm);settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
  result=(cap_measure if cap is not None else measure)(r,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,settings,source=r/arm,trace=trace,identity={'artifact':ids[arm+'_plugin'],'harness':sha(Path(__file__)),'protocol':sha(r/'memory-followup-protocol.md'),'usable_cap':cap,'kind':kind})
  row[arm]=result;paths[arm]=out;(r/'memory-followup-results.json').write_text(json.dumps(rows+[row],indent=2)+'\n');print(kind,arm,result['status'],result['seconds'],flush=True);assert result['status']=='pass',out;certify(out)
  row[arm+'_phase_lines']=[z for z in (out/'stderr.txt').read_text().splitlines() if z.startswith('FEREG_PHASE')]
  if cap is not None:
   assert result['metadata']['fereg_native_allocation_bytes']<=cap,(kind,arm,result['metadata']['fereg_native_allocation_bytes'],cap)
   row[arm+'_unrestricted_comparison']=baseline_compare(out,r/'runs'/('screen-low_weak-'+arm))
  if kind=='rank-profile':
   assert any('low_score_retention|retained=1' in z for z in row[arm+'_phase_lines'])
   for f in ['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','saved_outputs.csv']:assert (out/f).read_bytes()==(r/'runs'/('rank-robust-'+req['name']+'-'+arm)/f).read_bytes(),f
 if 'baseline' in paths:
  row['comparison']=baseline_compare(paths['candidate'],paths['baseline'])
  for f in ['fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv']:assert (paths['candidate']/f).read_bytes()==(paths['baseline']/f).read_bytes(),f
  cols=[0,1,2,3,10,11,12,13,14,15];assert np.array_equal(matrix(paths['candidate'],'fereg_solver')[:,cols],matrix(paths['baseline'],'fereg_solver')[:,cols]);row['solver_decisions_exact']=True
  if (paths['candidate']/'saved_outputs.csv').exists():
   a,b=pd.read_csv(paths['candidate']/'saved_outputs.csv'),pd.read_csv(paths['baseline']/'saved_outputs.csv');assert list(a)==list(b);assert np.allclose(a.to_numpy(float),b.to_numpy(float),atol=1e-9,rtol=1e-7,equal_nan=True)
  b,c=row['baseline']['seconds'],row['candidate']['seconds'];row.update(delta=c-b,percent=100*(c-b)/b,cost_flag=c-b>max(.01,.05*b))
 rows.append(row);(r/'memory-followup-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('LOW_SCORE_MEMORY_FOLLOWUP_TERMINAL_PASS',flush=True)
