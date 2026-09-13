import os
from pathlib import Path
import json,csv,statistics,hashlib
root=Path(__file__).resolve().parent;rows=json.loads((root/'cost-results.json').read_text());assert len(rows)==19 and (root/'cost-run.log').read_text().splitlines()[-1]=='COST_COMPARISONS_TERMINAL_PASS'
summary=[]
for r in rows:
 row={'name':r['name'],'accuracy_matched':r['accuracy_matched_timing'],'percent':r['percent'],'delta_seconds':r['delta'],'review':r['review']}
 for arm in ['baseline','candidate']:
  m=r[arm]['metadata'];p=root/'cost-runs'/(r['name']+'-'+arm)
  row[arm]={'command_seconds':r[arm]['seconds'],'native_bytes':m.get('fereg_native_allocation_bytes'),'public_base_bytes':m.get('fereg_memory_selected_bytes'),'scratch_bytes':m.get('fereg_scratch_bytes'),'rss_gib':r[arm]['rss_gib']}
  if (p/'fereg_solver.csv').exists():
   sol={(int(x['row']),int(x['column'])):float(x['value']) for x in csv.DictReader((p/'fereg_solver.csv').open())}
   runtime={(int(x['row']),int(x['column'])):float(x['value']) for x in csv.DictReader((p/'fereg_runtime.csv').open())}
   row[arm].update(input_s=runtime[19,2],native_s=runtime[20,2],runtime_total_s=runtime[21,2])
   for index,label in [(1,'initial'),(2,'final')]:row[arm][label]={key:sol[index,col] for col,key in [(1,'requested'),(2,'used'),(3,'iterations'),(6,'relative_inf'),(9,'setup_s'),(10,'solve_s'),(11,'fallback'),(12,'warning_count'),(15,'pilot_iterations')]}
 if 'independent_outputs' in r:row['independent_outputs']=r['independent_outputs'];row['independent_finite']=r['independent_finite']
 summary.append(row)
(root/'cost-review.json').write_text(json.dumps(summary,indent=2)+'\n')
print('original baseline output failures:')
for row in summary[:13]:
 print(row['name'],{col:res['failing_rows'] for col,res in row.get('independent_outputs',{}).get('baseline',{}).items()},'native_MB',row['baseline']['native_bytes']/1e6,row['candidate']['native_bytes']/1e6)
for name in ['slope','weighted_cut','auto_cmg_saved']:
 rr=[r for r in rows if r['source_name']==name];print(name,'median delta_ms',1000*statistics.median(r['delta'] for r in rr),'median paired pct',statistics.median(r['percent'] for r in rr))
