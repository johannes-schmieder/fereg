import os
from pathlib import Path
import csv,json
r=Path(__file__).resolve().parent;rows=json.loads((r/'baseline-route-results.json').read_text());assert len(rows)==3;summary=[]
for row in rows:
 out={'name':row['name'],'routes':{}}
 assert len(row['science'])==2
 for a in ['auto','map','cmg']:
  result=row[a];rt=row[a+'_runtime'];assert result['status']=='pass';assert rt['allocation_peak_delta']<=rt['modeled_selected_bytes']
  solver={(int(x['row']),int(x['column'])):None if x['value'].strip()=='.' else float(x['value']) for x in csv.DictReader((r/'baseline-runs'/(row['name']+'-'+a)/'feppml_solver.csv').open())}
  out['routes'][a]={'command_seconds':result['seconds'],'native_peak_delta':rt['allocation_peak_delta'],'modeled_selected_bytes':rt['modeled_selected_bytes'],'sampled_rss_gib':result['peak_rss_gib'],'scratch_bytes':rt['observed_scratch_bytes'],'outer_iterations':rt['outer_iterations'],'inner_iterations':rt['inner_iterations'],'step_halvings':rt['step_halvings'],'strict_profile_rejections':rt['strict_profile_rejections'],'fallback_count':rt['fallback_count'],'phases':{k:v for k,v in rt.items() if k.startswith('time_')},'solver_rows':{name:{label:solver[i,j] for j,label in [(8,'ordinary_frames'),(9,'checkpoint_frames'),(10,'retries'),(11,'fallbacks'),(13,'iterations'),(14,'setup_seconds'),(15,'solve_seconds'),(16,'certification_seconds')]} for i,name in enumerate(['map','lsmr_fused','lsmr_joint','cmg_prepared'],1)}}
 out['science']=row['science'];summary.append(out)
(r/'baseline-route-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
traces=json.loads((r/'checkpoint-trace-results.json').read_text());assert len(traces)==3;out=[]
for row in traces:
 assert all(row['exact_science'].values())
 parsed=[dict(v.split('=',1) for v in x.split('|')[1:]) for x in row['lines'] if 'checkpoint_lsmr_attempt' in x];maximum=max(int(x['rows']) for x in parsed);full=[x for x in parsed if int(x['rows'])==maximum];out.append({'name':row['name'],'compact_rows':maximum,'warm_attempts':len(parsed)-len(full),'full_attempts':full})
(r/'checkpoint-attempt-summary.json').write_text(json.dumps(out,indent=2)+'\n')
print('SCHUR_ROUTE_COLLECTION_TERMINAL_PASS')
