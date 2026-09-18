from pathlib import Path
import json,csv,statistics,sys,math
root=Path(sys.argv[1]);pairs=json.loads((root/'pairs.json').read_text());output=[]
def mat(path):
 with path.open() as s:return {(int(r['row']),int(r['column'])):float(r['value']) if r['value'].strip()!='.' else float('nan') for r in csv.DictReader(s)}
for case in sorted({r['case'] for r in pairs}):
 rs=[r for r in pairs if r['case']==case];arms=list(rs[0]['arms']);values={a:{} for a in arms}
 for row in rs:
  for a,r in row['arms'].items():
   def add(k,v):values[a].setdefault(k,[]).append(v)
   add('command_seconds',r['seconds']);add('rss_peak_bytes',r['peak_rss_gib']*2**30)
   prefix='feppml' if r['cell']['model']=='ppml' else 'fereg'
   d=root/'runs'/case/('p'+str(row['pair'])+'-'+a)
   m=mat(d/(prefix+'_runtime.csv'))
   if prefix=='fereg':
    for j,n in [(9,'scratch_bytes'),(13,'native_peak_bytes'),(18,'preflight_seconds'),(19,'input_seconds'),(20,'native_seconds'),(21,'plugin_seconds')]:add(n,m[j,2])
    solver=mat(d/'fereg_solver.csv')
    for j,n in [(9,'solver_setup_seconds'),(10,'solver_solve_seconds')]:add(n,sum(solver.get((rr,j),0) for rr in [1,2]))
    add('fallback_count',sum(solver.get((rr,11),0) for rr in [1,2]))
   else:
    for j,n in [(16,'scratch_bytes'),(21,'native_peak_bytes'),(9,'fallback_count')]:add(n,m[j,1])
    names=['preparation','separation_fe','separation_simplex','separation_ir','separation_mu','weight_response_refresh','fe_projection','finite_solve','objective_line_search','strict_final_solve','inference','fe_recovery','publication']
    for j,n in enumerate(names,28):add(n+'_seconds',m[j,1])
 med={a:{k:statistics.median([x for x in v if math.isfinite(x)]) if any(math.isfinite(x) for x in v) else None for k,v in values[a].items()} for a in arms}
 before='ordinary' if 'ordinary' in arms else 'baseline';after='components' if 'components' in arms else 'candidate'
 flags=[]
 for k in med[before]:
  b,c=med[before][k],med[after][k]
  if b is not None and c is not None and k.endswith('_seconds') and c-b>.01 and c>1.05*b:flags.append(k)
 output.append(dict(case=case,medians=med,timing_flags=flags))
(root/'phase-summary.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps([{ 'case':r['case'],'flags':r['timing_flags']} for r in output]))
