from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','full-pilot-cost-shuffled-unit','--widths','5','--shuffled','--seed','31415927'],
 ['--label','full-pilot-cost-shuffled-aw','--widths','6','--shuffled','--seed','31415927','--weight-kind','aw','--weight-design','moderate'],
]
for design in ['strong_d3','strong_d6','unbalanced']:
 for weight in ['none','aw']:
  cells.append(['--label','full-pilot-cost-'+design+'-'+weight,'--design',design,'--n','245760','--widths','6','--weight-kind',weight,'--weight-design','none' if weight=='none' else 'moderate'])
for cell in cells:
 with (root/(cell[1]+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','fusedreconstruction','fullpilotcost','--cases','iid','--repeats','1','--trace']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(cell[1],code,flush=True)
 if code:raise SystemExit(code)
print('FULL_PILOT_COST_SCREEN_COMPLETE',flush=True)
