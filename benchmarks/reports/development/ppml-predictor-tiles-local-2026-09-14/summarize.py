import os
from pathlib import Path
import json,statistics as s,hashlib,csv
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
screen=read('screen-results.json');follow=read('followup-results.json');cost=read('cost-review-results.json');variability=read('low-variability-results.json');diag=read('diagnostic-results.json')
def aggregate(rows):
    return {'pairs':len(rows),'baseline_s':s.median(q['baseline']['seconds'] for q in rows),'candidate_s':s.median(q['candidate']['seconds'] for q in rows),'median_delta_s':s.median(q['delta'] for q in rows),'median_percent':s.median(q['percent'] for q in rows),'deltas_s':[q['delta'] for q in rows],'percentages':[q['percent'] for q in rows],'cost_flags':sum(q['cost_flag'] for q in rows)}
result={'primary':{},'controls':{},'diagnostics':{},'cost_review':{}}
for first in screen:
    name=first['name'];rows=[first]+[q for q in follow if q['name']==name]
    assert len(rows)==3
    result['primary'][name]=aggregate(rows)
    result['primary'][name]['phases_and_memory']={a:{k:s.median(q['runtime'][a][k] for q in rows) for k in ['time_preparation','time_weight_response_refresh','time_fe_projection','time_finite_solve','time_strict_final_solve','time_inference','time_publication','allocation_peak_delta','modeled_selected_bytes','modeled_scratch_bytes','observed_scratch_bytes']} for a in ['baseline','candidate']}
    result['primary'][name]['rss_gib']={a:s.median(q[a]['peak_rss_gib'] for q in rows) for a in ['baseline','candidate']}
for q in follow:
    if q['name'] not in result['primary']:result['controls'][q['name']]=aggregate([q])
for kind in ['AA','AB']:result['cost_review'][kind]=aggregate([q for q in variability if q['kind']==kind])
low=[q for q in follow if q['name']=='low_fw']+cost+[q for q in variability if q['kind']=='AB']
result['cost_review']['all_nine_AB']=aggregate(low)
for q in diag:
    result['diagnostics'][q['name']]={}
    for a in ['baseline','candidate']:
        rows=[dict(p.split('=',1) for p in line.split('|')[2:]) for line in q[a+'_predictor_lines']]
        n=max(int(v['rows']) for v in rows);full=[v for v in rows if int(v['rows'])==n]
        result['diagnostics'][q['name']][a]={'rows':n,'calls':len(full),'warm_calls':len(rows)-len(full),**{k:sum(float(v[k]) for v in full) for k in ['initialize','accumulate','certificate','total']}}
all_rows=screen+follow+cost+variability
for q in all_rows:
    assert q['baseline']['status']==q['candidate']['status']=='pass'
    for a in ['baseline','candidate']:
        rt=q['runtime'][a];assert rt['allocation_peak_delta']<=rt['modeled_selected_bytes'],(q['name'],a)
    assert q['runtime']['baseline']['allocation_peak_delta']==q['runtime']['candidate']['allocation_peak_delta'],q['name']
result['counts']={'untraced_AB_pairs':25,'untraced_AA_pairs':6,'diagnostic_pairs':3,'total_command_calls':68,'core_tests':20,'serial_plugin_tests':95,'independent_licensed_fits_per_arm':4}
result['decision']='Adopt unchanged original candidate. Primary gains are repeated and changed-predictor timing is reduced. Low-FW individual cost flags remain; interleaved A/A spans -132 to +185 ms, new AB median -24 ms and all-nine AB median +3 ms. No low-mode speedup or zero-overhead claim; no mode-specific guard.'
(r/'adoption-summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
