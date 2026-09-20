from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','fused-reconstruction-shuffled-unit','--widths','3','5','6','--shuffled','--seed','31415927','--cases','iid'],
 ['--label','fused-reconstruction-shuffled-aw','--widths','6','--shuffled','--seed','31415927','--weight-kind','aw','--weight-design','moderate','--cases','iid','iid_outputs','--record-reference-output-disagreement'],
 ['--label','fused-reconstruction-ordered-aw','--widths','6','50','--weight-kind','aw','--weight-design','moderate','--cases','iid','iid_outputs','--record-reference-output-disagreement'],
]
for cell in cells:
 with (root/(cell[1]+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightedstatistics','fusedreconstruction','fixest','--repeats','3']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(cell[1],code,flush=True)
 if code:raise SystemExit(code)
print('FUSED_RECONSTRUCTION_SCREEN_COMPLETE',flush=True)
