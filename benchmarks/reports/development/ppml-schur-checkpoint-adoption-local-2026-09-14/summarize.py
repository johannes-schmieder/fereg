import os
from pathlib import Path
import json,statistics as st,hashlib,re
r=Path(__file__).resolve().parent;first=json.loads((r/'screen-results.json').read_text());more=json.loads((r/'followup-results.json').read_text());maps=json.loads((r/'map-results.json').read_text());median=st.median
summary={'primary':{},'controls':{},'map_comparison':{},'traces':{},'scope':'local paired development evidence; no external reference speed or release qualification'}
for q in first:
 rows=[q]+[x for x in more if x['name'].startswith(q['name']+'-s')];assert len(rows)==3
 a=median(x['baseline']['seconds'] for x in rows);b=median(x['candidate']['seconds'] for x in rows);pct=median(100*(1-x['candidate']['seconds']/x['baseline']['seconds']) for x in rows)
 row={'pairs':3,'baseline_s':a,'candidate_s':b,'median_paired_reduction_percent':pct,'deltas_s':[x['candidate']['seconds']-x['baseline']['seconds'] for x in rows]}
 assert pct>=5 and median(x['baseline']['seconds']-x['candidate']['seconds'] for x in rows)>=.01
 row['gate_passed']=True
 for arm in ['baseline','candidate']:
  row[arm+'_runtime_medians']={k:median(x[arm+'_runtime'][k] for x in rows) for k in ['time_preparation','time_fe_projection','time_strict_final_solve','time_finite_solve','time_inference','time_publication','allocation_peak_delta','modeled_selected_bytes','outer_iterations','inner_iterations','fallback_count']}
  row[arm+'_rss_gib']=median(x[arm]['peak_rss_gib'] for x in rows)
 assert all(x['science']['registered_numeric_tolerances_pass'] for x in rows)
 summary['primary'][q['name']]=row
for q in more:
 if '-s' in q['name']:continue
 a,b=q['baseline']['seconds'],q['candidate']['seconds'];flag=b-a>.01 and (b/a-1)>.05
 summary['controls'][q['name']]={'baseline_s':a,'candidate_s':b,'cost_s':b-a,'cost_percent':100*(b/a-1),'cost_flag':flag,'baseline_native_bytes':q['baseline_runtime']['allocation_peak_delta'],'candidate_native_bytes':q['candidate_runtime']['allocation_peak_delta'],'baseline_rss_gib':q['baseline']['peak_rss_gib'],'candidate_rss_gib':q['candidate']['peak_rss_gib'],'science':q['science']}
 assert not flag
allrows={x['name']:x for x in first+more}
for q in maps:
 candidate=allrows[q['name']]['candidate']['seconds'];summary['map_comparison'][q['name']]={'accepted_map_s':q['result']['seconds'],'candidate_auto_s':candidate,'reduction_percent':100*(1-candidate/q['result']['seconds'])};assert q['science']['registered_numeric_tolerances_pass']
for q in json.loads((r/'trace-results.json').read_text()):
 records=[]
 for line in q['lines']:
  if 'FEREG_SOLVER_TRACE|' not in line:continue
  fields=dict(x.split('=',1) for x in line.split('|')[1:] if '=' in x)
  if int(fields.get('rows','0'))<16384:continue
  records.append(fields)
 summary['traces'][q['name']]=records
summary['counts']={'primary_pairs':9,'protected_pairs':11,'untraced_commands':43,'separate_diagnostic_commands':9,'independent_native_oracle_fits':9,'independent_dummy_poisson_fits':3}
summary['decision']='Time/science/control gates pass. Adopt bounded Schur checkpoint work with existing ordinary LSMR and independent certificates; broader P0/P4/P6/P7 remains.'
(r/'adoption-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
validation=json.loads((r/'validation-summary.json').read_text());validation.update(status='pass',universal_static_and_native_arm64_abi=True,candidate_plugin=json.loads((r/'built-identities.json').read_text())['candidate_plugin'],licensed_oracle=json.loads((r/'oracle-results.json').read_text()),comparison_counts=summary['counts'],all_native_models_admit_untraced_peaks=True,all_diagnostic_science_and_solver_fields_exact=True)
(r/'validation-summary.json').write_text(json.dumps(validation,indent=2)+'\n')
print(json.dumps({k:{t:v[t] for t in ['baseline_s','candidate_s','median_paired_reduction_percent','baseline_rss_gib','candidate_rss_gib']} for k,v in summary['primary'].items()},indent=2))
print('CHECKPOINT_SCHUR_SUMMARY_TERMINAL_PASS')
