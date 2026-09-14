import os
from pathlib import Path
import json,hashlib
r=Path(__file__).resolve().parent
rows=json.loads((r/'screen-results.json').read_text());assert len(rows)==4
out=[]
for x in rows:
 a,b=x['runtime']['baseline'],x['runtime']['candidate']
 assert x['baseline']['status']==x['candidate']['status']=='pass'
 assert all(x['science'].values())
 assert a['allocation_peak_delta']==b['allocation_peak_delta']
 assert a['modeled_selected_bytes']==b['modeled_selected_bytes']
 assert a['allocation_peak_delta']<=a['modeled_selected_bytes']
 assert b['allocation_peak_delta']<=b['modeled_selected_bytes']
 assert all(a[k]==b[k] for k in a if k.startswith('modeled_'))
 out.append({'name':x['name'],'seconds':{arm:x[arm]['seconds'] for arm in ['baseline','candidate']},'paired_percent':x['percent'],'time_only_gate':x['qualifying_time_gain'],'cost_flag':x['cost_flag'],'mean_refresh_seconds':{arm:x['runtime'][arm]['time_weight_response_refresh'] for arm in ['baseline','candidate']},'native_peak_delta':a['allocation_peak_delta'],'modeled_selected':a['modeled_selected_bytes'],'rss_gib':{arm:x[arm]['peak_rss_gib'] for arm in ['baseline','candidate']},'science':x['science'],'iterations':{'outer':a['outer_iterations'],'inner':a['inner_iterations'],'halvings':a['step_halvings'],'fallbacks':a['fallback_count'],'strict_rejections':a['strict_profile_rejections']}})
(r/'screen-summary.json').write_text(json.dumps(out,indent=2)+'\n')
p=r/'diagnostic-results.json'
if p.exists():
 rows=json.loads(p.read_text());assert len(rows)==3;result=[]
 for x in rows:
  row={'name':x['name']}
  for arm in ['baseline','candidate']:
   parsed=[dict(part.split('=',1) for part in line.split('|')[2:]) for line in x[arm+'_mean_lines']]
   max_rows=max(int(z['rows']) for z in parsed);full=[z for z in parsed if int(z['rows'])==max_rows]
   sizes={int(z['workspace_size']) for z in parsed};assert len(sizes)==1
   tag_sizes={int(z['bounds_option_size']) for z in parsed};assert len(tag_sizes)==1
   row[arm]={'full_rows':max_rows,'full_refresh_calls':len(full),'full_reused_calls':sum(int(z['reused']) for z in full),'full_mean_seconds':sum(float(z['seconds']) for z in full),'warm_calls':len(parsed)-len(full),'workspace_size':sizes.pop(),'bounds_option_size':tag_sizes.pop()}
  assert row['baseline']['full_refresh_calls']==row['candidate']['full_refresh_calls']
  row['mean_seconds_saved']=row['baseline']['full_mean_seconds']-row['candidate']['full_mean_seconds']
  command=next(z['seconds']['baseline'] for z in out if z['name']==x['name'])
  row['baseline_mean_share_of_untraced_command_percent']=100*row['baseline']['full_mean_seconds']/command
  result.append(row)
 (r/'diagnostic-summary.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result,indent=2),flush=True)
print('MEAN_COLLECTION_TERMINAL_PASS',flush=True)
