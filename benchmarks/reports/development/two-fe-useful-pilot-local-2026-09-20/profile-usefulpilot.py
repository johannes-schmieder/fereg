from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
for name,extra in [
 ('shuffled-unit',['--widths','5','--shuffled','--seed','31415927']),
 ('shuffled-aw',['--widths','6','--shuffled','--seed','31415927','--weight-kind','aw','--weight-design','moderate']),
 ('strong6-unit',['--widths','6','--design','strong_d6','--n','245760']),
 ('strong6-aw',['--widths','6','--design','strong_d6','--n','245760','--weight-kind','aw','--weight-design','moderate'])]:
 label='useful-pilot-profile-'+name
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','fusedreconstruction','usefulpilot','--repeats','1','--cases','iid','--trace']+extra,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('USEFUL_PILOT_PROFILE_COMPLETE',flush=True)
