from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
for name,extra in [('unit',['--widths','5']),('aw',['--widths','6','--weight-kind','aw','--weight-design','moderate'])]:
 label='fused-reconstruction-profile-'+name
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','weightedstatistics','fusedreconstruction','--repeats','1','--cases','iid','--shuffled','--seed','31415927','--trace']+extra,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('FUSED_RECONSTRUCTION_PROFILE_COMPLETE',flush=True)
