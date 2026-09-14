import os
from pathlib import Path
import json,csv,math,hashlib
root=Path(__file__).resolve().parent;rows=json.loads((root/'cost-results.json').read_text());out=[]
for x in rows:
 if x['name'] not in ['disconnected_blocks','unbalanced','auto_cmg_saved','weak_d3','weighted_cut']:continue
 m=x['candidate']['metadata'];p=root/'cost-runs'/(x['name']+'-candidate');n=int(m['N']);t=int(m['fereg_threads'])
 generated=max(int(z['row']) for z in csv.DictReader((p/'fereg_status.csv').open()));k=generated+1
 f={(int(z['row']),int(z['column'])):float(z['value']) for z in csv.DictReader((p/'fereg_fe.csv').open())};levels=[int(f[i,4]) for i in [1,2]];l,h=max(levels),min(levels)
 s={(int(z['row']),int(z['column'])):float(z['value']) for z in csv.DictReader((p/'fereg_solver.csv').open())};accepted=s[1,15]==8 and s[1,16]<=.01
 cg=3*n*k*8+l*k*16+max(math.ceil(n/32768),t)*k*8+k*128
 schur=8*max(l+h+(2*l+4*h)*k+12*k+(l*k+l+math.ceil(n/512)*k if t>1 else 0),l+h+(2*l+4*h)*3)
 base=int(m['fereg_memory_selected_bytes']);peak=int(m['fereg_native_allocation_bytes']);joint=base+max(cg,schur)
 row=dict(name=x['name'],n=n,generated_columns=generated,rhs=k,threads=t,levels=levels,accepted_probe=accepted,base=base,initial_schur_cg_joint=joint,native_peak=peak,whole_peak_within_initial_joint=peak<=joint)
 out.append(row);print(json.dumps(row))
(root/'control-admission-replay.json').write_text(json.dumps({'method':'Replay initial Schur/CG admission only. A whole-action peak above this bound may arise in a later independently admitted phase and requires tracing. No universal or final/recovery admission claim.','source_sha256':{p:hashlib.sha256((root/'candidate'/p).read_bytes()).hexdigest() for p in ['crates/fereg-core/src/fe/borrowed_map.rs','crates/fereg-core/src/memory.rs','crates/fereg-core/src/ols/prepared_fit.rs']},'rows':out},indent=2)+'\n')
