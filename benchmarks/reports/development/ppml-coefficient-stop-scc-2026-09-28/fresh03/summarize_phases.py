"""Read recorded native telemetry only; never run fits or combine phase clocks."""
from pathlib import Path
import json,hashlib,sys,statistics
root,out=map(Path,sys.argv[1:]);plan=json.loads((root/'plan.json').read_text())
fields=['seconds','ic','inner_iterations','feppml_time_preparation','feppml_time_fe_projection','feppml_time_finite_solve','feppml_time_strict_final_solve','feppml_time_inference','feppml_time_line_search','feppml_time_weight_refresh','feppml_time_publication','feppml_allocation_peak','feppml_memory_selected_bytes','feppml_strict_profile_rejections']
def dist(v):return dict(values=v,min=min(v),median=statistics.median(v),max=max(v))
cases=[];bindings={}
for case in plan['cases']:
 arms={}
 for arm in ['baseline','candidate']:
  records=[]
  for rep in range(1,len(case['seeds'])+1):
   p=root/case['id']/f'pair-{rep}'/arm/'result.json';d=json.loads(p.read_text());bindings[str(p.relative_to(root))]=hashlib.sha256(p.read_bytes()).hexdigest();assert d['status']=='pass'
   records.append(d['metadata'])
  arms[arm]={key:dist([r[key] for r in records]) for key in fields if all(isinstance(r.get(key),(float,int)) for r in records)}
 cases.append(dict(case=case['id'],arms=arms))
out.write_text(json.dumps(dict(scope='Native published telemetry from recorded headline fits; phase clocks may nest and must not be summed or treated as independent causal contributions. Iteration definitions are native only, not comparable stock operations.',cases=cases,bindings=bindings),indent=2)+'\n')
