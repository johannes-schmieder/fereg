import os
from pathlib import Path
import json,csv,math,hashlib
root=Path(__file__).resolve().parent;repo=Path(os.environ['REPOSITORY_ROOT']);rows=json.loads((root/'cost-results.json').read_text());out=[]
for x in rows:
 if '_s' not in x['name'] or not x['name'].split('_s')[-1].isdigit():continue
 m=x['candidate']['metadata'];p=root/'cost-runs'/(x['name']+'-candidate');n=int(m['N']);t=int(m['fereg_threads'])
 status=list(csv.DictReader((p/'fereg_status.csv').open()));generated=max(int(z['row']) for z in status);k=generated+1
 assert generated==51 and k==52
 f={(int(z['row']),int(z['column'])):float(z['value']) for z in csv.DictReader((p/'fereg_fe.csv').open())};levels=[int(f[i,4]) for i in [1,2]];l,h=max(levels),min(levels)
 s={(int(z['row']),int(z['column'])):float(z['value']) for z in csv.DictReader((p/'fereg_solver.csv').open())};accepted=s[1,15]==8 and s[1,16]<=.01
 cg=3*n*k*8+l*k*16+max(math.ceil(n/32768),t)*k*8+k*128
 schur=8*max(l+h+(2*l+4*h)*k+12*k+l*k+l+math.ceil(n/512)*k,l+h+(2*l+4*h)*3)
 coefficient=sum(levels)*k*8
 planning=max(cg,schur);base=int(m['fereg_memory_selected_bytes']);required=base+(planning if accepted else cg+5*coefficient)
 row=dict(name=x['name'],n=n,generated_columns=generated,rhs=k,threads=t,levels=levels,probe=s[1,16],accepted_probe=accepted,base=base,initial_schur_cg_joint=base+planning,rejected_retained_cg_joint=base+cg+5*coefficient,actual_initial_route_joint=required,native_peak=int(m['fereg_native_allocation_bytes']),fits=int(m['fereg_native_allocation_bytes'])<=required,initial_iterations=s[1,3],initial_fallback=s[1,11]);out.append(row);assert row['fits']
 print(x['name'],'joint_MB',required/1e6,'peak_MB',row['native_peak']/1e6,'probe',row['probe'])
for name,bound in [('unit_saved_s104729',827753800),('pw_partial_s104729',831179336)]:assert next(x['initial_schur_cg_joint'] for x in out if x['name']==name)==bound
(root/'primary-admission-replay.json').write_text(json.dumps({'method':'Scalar replay of existing schur_ols_workspace_bytes and estimate_fe_solver_memory(CG) expressions, using captured finalized levels/N/worker counts and all 51 generated coordinates plus outcome. Cross-checked against both actually exercised exact/one-byte-short initial public caps. Not a new runtime memory trace or universal factor-storage bound.','corrected_diagnostic':'First draft assumed 51 RHS and a common 32768 row chunk. The captured shape/source use 52 RHS and Schur chunk 512 (CG chunk 32768). The exact-cap cross-check rejected that draft before reporting; all source values are now explicit. No fit, seed, tolerance or measured capture changed.','source_inputs':{p:hashlib.sha256((repo/p).read_bytes()).hexdigest() for p in ['crates/fereg-core/src/fe/borrowed_map.rs','crates/fereg-core/src/memory.rs','crates/fereg-core/src/ols/prepared_fit.rs']},'rows':out},indent=2)+'\n')
