import os
from pathlib import Path
import csv,json,subprocess,hashlib
r=Path(__file__).resolve().parent;ids=json.loads((r/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(r/'candidate'/p)==h for p,h in ids['candidate_inputs'].items())
p=r/'runs/screen-low_weak-candidate';m=json.loads((p/'result.json').read_text())['metadata'];f={(int(x['row']),int(x['column'])):float(x['value']) for x in csv.DictReader((p/'fereg_fe.csv').open())};levels=[int(f[i,4]) for i in sorted({x[0] for x in f})];n=int(m['N']);t=int(m['fereg_threads']);pool=int(m['fereg_poolsize']);base=int(m['fereg_memory_selected_bytes']);rows=[]
for rhs in [1,pool]:
 z=json.loads(subprocess.check_output([str(r/'admission_probe'),str(n),str(rhs),str(t)]+list(map(str,levels)),text=True));z.update(n=n,rhs=rhs,threads=t,levels=levels,execution_base=base);rows.append(z)
caps={'lsmr':base+max(x['lsmr_schwarz_bytes'] for x in rows),'cg':base+max(x['cg_bytes'] for x in rows)}
(r/'low-admission-replay.json').write_text(json.dumps({'case':'low_weak','rows':rows,'usable_caps':caps,'scope':'Current borrowed initial/final absorption plans for up-to-pool-width batches. Not public whole-action reporting or a universal allocation bound.','source_sha256':{p:sha(r/'candidate'/p) for p in ['crates/fereg-core/src/memory.rs','crates/fereg-core/src/fe/borrowed_map.rs','crates/fereg-core/src/ols/prepared_fit.rs']}},indent=2)+'\n');print(caps)
