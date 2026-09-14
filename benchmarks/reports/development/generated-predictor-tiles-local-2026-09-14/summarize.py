import os
from pathlib import Path
import os,sys,json,hashlib,re,statistics as st,csv
r=Path(__file__).resolve().parent
sys.path[:0]=[str(r/'harness')]
from ppml_preparation_compare import compare as compare_exact,named_runtime
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((r/'built-identities.json').read_text());repo=Path(os.environ['REPOSITORY_ROOT'])
for arm in ['baseline','candidate']:
 assert all(sha(r/arm/p)==h for p,h in ids[arm+'_inputs'].items())
 assert sha(r/arm/'dist/fereg_macos.plugin')==ids[arm+'_plugin']
assert all(sha(repo/p)==h for p,h in ids['candidate_inputs'].items())
assert sha(repo/'dist/fereg_macos.plugin')==ids['installed_dist']
pairs=[];summary=[]
for kind in ['screen','followup']:
 for row in json.loads((r/(kind+'-results.json')).read_text()):
  paths={a:r/(kind+'-runs')/(row['name']+'-'+a) for a in ['baseline','candidate']}
  passed=all(row[a]['status']=='pass' for a in paths)
  if passed:
   compare_exact(paths)
   rt={a:named_runtime(p) for a,p in paths.items()}
   assert all(rt[a]['allocation_peak_delta']<=rt[a]['modeled_selected_bytes'] for a in paths)
   b,c=[row[a]['seconds'] for a in paths]
   item={'name':row['name'],'baseline':b,'candidate':c,'saved_seconds':b-c,'reduction_percent':100*(b-c)/b,'runtime':rt,'rss_gib':{a:row[a]['peak_rss_gib'] for a in paths},'exact_science':True,'native_peak_difference':rt['candidate']['allocation_peak_delta']-rt['baseline']['allocation_peak_delta']}
  else:
   assert row['name']=='topology_rank-s31415927'
   for p in paths.values():assert 'SYSTEMATIC_WARMUP_FAILED rc=430' in (p/'import.log').read_text()
   item={'name':row['name'],'status':'both warmups fail r(430), no target timing','exact_science':None}
  pairs.append(item)
  for a,p in paths.items():
   q=row[a];out={'kind':kind,'name':row['name'],'arm':a,'status':q['status'],'command_seconds':q['seconds'],'rss_gib':q['peak_rss_gib']}
   if q['status']=='pass':out.update(named_runtime(p))
   summary.append(out)
primary={}
for label in ['ppml_fw','generated_unit','generated_pw']:
 ps=[p for p in pairs if p['name'].split('-s')[0]==label];assert len(ps)==3
 fields=['total_seconds','time_preparation','time_weight_response_refresh','time_strict_final_solve','time_fe_projection','time_finite_solve','time_inference','allocation_peak_delta','modeled_selected_bytes']
 primary[label]={'baseline_median':st.median(p['baseline'] for p in ps),'candidate_median':st.median(p['candidate'] for p in ps),'median_paired_reduction_percent':st.median(p['reduction_percent'] for p in ps),'median_paired_saved_seconds':st.median(p['saved_seconds'] for p in ps),'rss_gib':{a:st.median(p['rss_gib'][a] for p in ps) for a in ['baseline','candidate']},'runtime_medians':{k:{a:st.median(p['runtime'][a][k] for p in ps) for a in ['baseline','candidate']} for k in fields if k in ps[0]['runtime']['baseline']}}
 assert primary[label]['median_paired_reduction_percent']>=5 and primary[label]['median_paired_saved_seconds']>=.01
controls=[p for p in pairs if p['name'].split('-s')[0] not in primary and p.get('exact_science')]
flags=[p for p in controls if p['reduction_percent'] < -5 and p['saved_seconds']<-.01];assert not flags
worst=min(controls,key=lambda p:p['saved_seconds'])
diag={}
for a in ['baseline','candidate']:
 p=r/(a+'-diagnostic');compare_exact({'baseline':r/'screen-runs'/('ppml_fw-'+a),'candidate':p})
 lines=json.loads((r/(a+'-diagnostic.json')).read_text())['predictor_lines'];full=[dict(field.split('=',1) for field in line.split('|')[2:]) for line in lines if 'rows=122382|' in line]
 assert len(full)==3
 diag[a]={'rows':122382,'calls':3,'accumulate_seconds':sum(float(q['accumulate']) for q in full),'certificate_seconds':sum(float(q['certificate']) for q in full),'initialize_seconds':sum(float(q['initialize']) for q in full)}
 out={'kind':'diagnostic','name':'ppml_fw','arm':a,'status':'pass','command_seconds':json.loads((p/'result.json').read_text())['seconds'],'rss_gib':json.loads((p/'result.json').read_text())['peak_rss_gib']};out.update(named_runtime(p));summary.append(out)
for q in json.loads((r/'boundary-results.json').read_text()):
 row={'kind':'boundary','name':q['label'],'arm':q['arm'],'status':q['result']['status'],'command_seconds':q['result']['seconds'],'rss_gib':q['result']['peak_rss_gib'],'available_bytes':q['available_bytes'],'return_code':q['result']['metadata'].get('rc')}
 if q['label']=='short':assert row['return_code']==909 and q['result']['sentinel']
 else:row.update(q['runtime']);assert q['runtime']['allocation_peak_delta']<=q['runtime']['modeled_selected_bytes']
 summary.append(row)
tests=json.loads((r/'test-results.json').read_text());names=set()
for t in tests:
 assert t['rc']==0
 if t['name']!='plugin-serial':names.update(re.findall(r'^test ([^ ]+) \.\.\. ok$',(r/(t['name']+'-tests.log')).read_text(),re.M))
assert len(names)==16
validation={'distinct_core_tests':len(names),'core_test_names':sorted(names),'serial_plugin_tests':95,'independent_fits_per_artifact':7,'passing_untraced_pairs':23,'unchanged_failed_warmup_pairs':1,'separate_diagnostic_pairs':1,'public_boundary_calls':4,'original_topology_warm_full_failure_atomicity_per_artifact':2,'source_inputs':len(ids['candidate_inputs']),'installed_dist_preserved':True}
for name,count in [('oracle-results.json',5),('weights-oracle-results.json',2)]:
 rows=json.loads((r/name).read_text());assert len(rows)==2 and all(q['sentinel'] and q['terminal_rc']==['0'] and q['process_rc']==0 and len(q['fits'])==count for q in rows)
for q in json.loads((r/'topology-failure-atomicity-results.json').read_text()):assert q['sentinel'] and q['terminal_rc']==['0']
result={'disposition':'adopt generated predictor tiles; retain baseline canonical-rank failure as failed coverage','primary':primary,'worst_successful_control':{k:v for k,v in worst.items() if k not in ['runtime','rss_gib']},'control_flags':flags,'diagnostic':diag,'validation':validation,'pairs':pairs}
(r/'adoption-summary.json').write_text(json.dumps(result,indent=2)+'\n')
keys=list(dict.fromkeys(k for q in summary for k in q))
with (r/'summary.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=keys);w.writeheader();w.writerows(summary)
print(json.dumps({k:v for k,v in result.items() if k!='pairs'},indent=2));print('GENERATED_PREDICTOR_SUMMARY_VERIFIED')
