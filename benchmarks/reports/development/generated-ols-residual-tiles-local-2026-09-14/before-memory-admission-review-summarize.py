import os
from pathlib import Path
import os,sys,json,statistics as st,csv,hashlib,re
r=Path(__file__).resolve().parent;sys.path.insert(0,str(r/'harness'))
from collect_preparation import runtime
from ppml_preparation_compare import named_runtime,compare as compare_exact
from final_compare import matrix
import numpy as np
rows=[];pairs=[]
for kind in ['screen','followup']:
 for q in json.loads((r/(kind+'-results.json')).read_text()):
  ps={a:r/(kind+'-runs')/(q['name']+'-'+a) for a in ['baseline','candidate']};assert all(q[a]['status']=='pass' for a in ps);ppml=q['baseline']['cell']['model']=='ppml';bs={}
  assert q['baseline']['sample_sha256']==q['candidate']['sample_sha256']
  if ppml:compare_exact(ps)
  else:
   exports=[p.name for p in ps['baseline'].glob('*.csv') if p.stem not in ['fereg_solver','fereg_runtime','feppml_solver','feppml_runtime']]
   assert all((ps['baseline']/name).read_bytes()==(ps['candidate']/name).read_bytes() for name in exports)
   cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15];assert np.array_equal(matrix(ps['baseline'],'fereg_solver')[:,cols],matrix(ps['candidate'],'fereg_solver')[:,cols])
  for a,p in ps.items():
   x={'kind':kind,'name':q['name'],'arm':a,'model':'ppml' if ppml else 'ols','command_seconds':q[a]['seconds'],'rss_gib':q[a]['peak_rss_gib'],'status':'pass'}
   if ppml:
    rt=named_runtime(p);x.update(native_peak=rt['allocation_peak_delta'],modeled=rt['modeled_selected_bytes']);x.update({k:v for k,v in rt.items() if k.startswith('time_') or 'iteration' in k or 'fallback' in k})
   else:
    rt=runtime(p/'fereg_runtime.csv');x.update(native_peak=rt[13],modeled=rt[25],input_seconds=rt[19],native_seconds=rt[20],outer_seconds=q[a]['seconds']-rt[21],scratch_bytes=rt[9]);sol=matrix(p,'fereg_solver')
    for i,label in enumerate(['initial','final']):
     for col,name in [(2,'iterations'),(8,'setup_seconds'),(9,'solve_seconds'),(10,'fallback'),(14,'probe_iterations')]:x[label+'_'+name]=float(sol[i,col])
   assert x['native_peak']<=x['modeled'],x
   bs[a]=x;rows.append(x)
  b,c=[q[a]['seconds'] for a in ps];pairs.append({'name':q['name'],'baseline':bs['baseline'],'candidate':bs['candidate'],'saved_seconds':b-c,'reduction_percent':100*(b-c)/b,'exact_science_and_solver':True})
primary={}
for name in ['ols_fast_aw','generated_unit','generated_pw']:
 a=[q for q in pairs if q['name'].split('-s')[0]==name];assert len(a)==3
 item={'baseline_median':st.median(q['baseline']['command_seconds'] for q in a),'candidate_median':st.median(q['candidate']['command_seconds'] for q in a),'median_paired_saved_seconds':st.median(q['saved_seconds'] for q in a),'median_paired_reduction_percent':st.median(q['reduction_percent'] for q in a),'phase_memory_rss_medians':{k:{arm:st.median(q[arm][k] for q in a) for arm in ['baseline','candidate']} for k in ['input_seconds','native_seconds','outer_seconds','native_peak','modeled','rss_gib']}}
 item['gain_gate_pass']=item['median_paired_saved_seconds']>=.01 and item['median_paired_reduction_percent']>=5;primary[name]=item
controls=[q for q in pairs if q['name'].split('-s')[0] not in primary];flags=[q for q in controls if q['saved_seconds']<-.01 and q['reduction_percent']< -5];assert not flags
peaks=[{'name':q['name'],'difference_bytes':q['candidate']['native_peak']-q['baseline']['native_peak']} for q in pairs if q['candidate']['native_peak']!=q['baseline']['native_peak']]
result={'disposition':'reject standalone prototype: PW misses predeclared repeated primary gain gate','primary':primary,'failed_primary_gates':[n for n,q in primary.items() if not q['gain_gate_pass']],'protected_flags':flags,'largest_added_control':min(controls,key=lambda q:q['saved_seconds']),'peak_differences':peaks,'pairs':pairs,'validation':{'distinct_core_tests':16,'serial_plugin_tests':95,'independent_fits_per_artifact':8,'all_25_untraced_pairs_exact':True,'public_memory_boundary_run':False,'additional_repeats_after_gate_failure':False}}
assert result['failed_primary_gates']==['generated_pw']
(r/'decision.json').write_text(json.dumps(result,indent=2)+'\n')
keys=list(dict.fromkeys(k for q in rows for k in q))
with (r/'summary.csv').open('w') as f:w=csv.DictWriter(f,fieldnames=keys);w.writeheader();w.writerows(rows)
print(json.dumps({k:v for k,v in result.items() if k not in ['pairs','largest_added_control']},indent=2));print('RESIDUAL_PROTOTYPE_REJECTION_VERIFIED')
