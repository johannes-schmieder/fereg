import os
from pathlib import Path
import json,csv,hashlib
import numpy as np
from ppml_one_fe_compare import numeric
from ppml_preparation_compare import compare as exact_ppml,named_runtime
from collect_preparation import runtime
r=Path(__file__).resolve().parent;rows=json.loads((r/'screen-results.json').read_text());summary=[]
for kind in ['screen-runs','trace-runs']:
 paths={a:r/kind/('ppml_fw-'+a) for a in ['baseline','candidate']};exact_ppml(paths)
 for arm in paths:
  x=named_runtime(paths[arm]);assert x['allocation_peak_delta']<=x['modeled_selected_bytes']
for row in rows:
 b=row['baseline']['seconds'];c=row['candidate']['seconds'];d={'name':row['name'],'baseline_seconds':b,'candidate_seconds':c,'change_percent':100*(c/b-1),'saved_ms':1000*(b-c),'prospective_gain_pass':c<=.95*b and b-c>=.01}
 for arm in ['baseline','candidate']:
  result=row[arm];p=r/'screen-runs'/(row['name']+'-'+arm)
  if row['name']=='ppml_fw':
   rt=named_runtime(p);d[arm+'_native_peak']=rt['allocation_peak_delta'];d[arm+'_modeled']=rt['modeled_selected_bytes'];d[arm+'_runtime']=rt
  else:
   rt=runtime(p/'fereg_runtime.csv');d[arm+'_native_peak']=rt[13];d[arm+'_modeled']=rt[25];d[arm+'_input_seconds']=rt[19];d[arm+'_native_seconds']=rt[20]
  assert d[arm+'_native_peak']<=d[arm+'_modeled'];d[arm+'_rss_gib']=result['peak_rss_gib']
 assert d['baseline_native_peak']==d['candidate_native_peak'];summary.append(d)
assert not any(x['prospective_gain_pass'] for x in summary)
report={'disposition':'reject standalone generated RHS tiles; no screen target meets prospective 5% AND 10 ms command gain','primary_pairs':3,'diagnostic_pairs':3,'source_changes_not_adopted':True,'summary':summary,'core_tests':18,'plugin_tests':95,'independent_oracle_fits_per_arm':5,'notes':['Exact-science diagnostic exports match each same-artifact untraced call.','Fast AW full-data RHS diagnostic: 31.432375 to 25.121333 ms; 6.311042 ms saved, baseline scope only 2.81% of untraced baseline command.','Low score-retention diagnostic is not an isolated RHS timer. Public PPML per-phase metadata remains available; no direct PPML RHS subtotal was instrumented.','No extra seeds, broad holdout timings, private iteration audit or exact public memory cap calls after the screen rejection.','Initial test fixture compilation/base-policy/rebased-axis errors are preserved; initial licensed PPML factor-slot mapping failure was corrected before screen measurement. Full explicit-dummy Hessian/sandwich and omitted-zero checks retain original tolerances.']}
(r/'decision.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps([{k:v for k,v in x.items() if k in ['name','change_percent','saved_ms','baseline_native_peak','candidate_native_peak']} for x in summary],indent=2))
