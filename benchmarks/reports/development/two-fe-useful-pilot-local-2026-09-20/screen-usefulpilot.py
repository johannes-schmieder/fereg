from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','useful-pilot-shuffled-unit','--widths','1','3','5','6','--shuffled','--seed','31415927','--cases','iid'],
 ['--label','useful-pilot-shuffled-aw','--widths','6','--shuffled','--seed','31415927','--weight-kind','aw','--weight-design','moderate','--cases','iid','iid_outputs','--record-reference-output-disagreement'],
 ['--label','useful-pilot-ordered-aw','--widths','6','50','--weight-kind','aw','--weight-design','moderate','--cases','iid','iid_outputs','--record-reference-output-disagreement'],
]
for design in ['strong_d3','strong_d6','unbalanced']:
 for weight in ['none','aw']:
  cells.append(['--label','useful-pilot-'+design+'-'+weight,'--design',design,'--n','245760','--widths','6','--weight-kind',weight,'--weight-design','none' if weight=='none' else 'moderate','--cases','iid'])
for cell in cells:
 with (root/(cell[1]+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','fusedreconstruction','usefulpilot','fixest','--repeats','3']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(cell[1],code,flush=True)
 if code:raise SystemExit(code)
print('USEFUL_PILOT_SCREEN_COMPLETE',flush=True)
