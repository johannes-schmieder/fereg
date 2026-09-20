from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','weighted-statistics-aw-shuffled','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--cases','iid'],
 ['--label','weighted-statistics-aw-strong6','--weight-kind','aw','--weight-design','moderate','--design','strong_d6','--n','245760','--cases','iid'],
 ['--label','weighted-statistics-fw','--weight-kind','fw','--weight-design','moderate','--cases','iid'],
 ['--label','weighted-statistics-pw','--weight-kind','pw','--weight-design','moderate','--cases','robust'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightedboth','weightedstatsprofile','--widths','6','--repeats','1','--trace']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_STATISTICS_PROFILE_COMPLETE',flush=True)
