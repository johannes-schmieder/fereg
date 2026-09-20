from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[('dispersed',['--weight-design','dispersed','--cases','iid_outputs']),('low',['--weight-design','moderate','--memory','low','--cases','iid']),('sparse-aw',['--weight-design','moderate','--design','two_fe_sparse','--cases','iid'])]
for name,extra in cells:
 label='useful-pilot-confirm-'+name
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','fusedreconstruction','usefulpilot','--widths','6','--repeats','3','--weight-kind','aw','--n','245760']+extra,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('USEFUL_PILOT_CONFIRM_COMPLETE',flush=True)
