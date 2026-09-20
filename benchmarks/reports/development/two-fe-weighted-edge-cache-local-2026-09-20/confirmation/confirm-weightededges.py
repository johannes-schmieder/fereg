from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920'); python='target/one-fe-profile-20260919/python/bin/python'
for name,args in [
 ('dispersed',['--weight-design','dispersed','--cases','iid_outputs']),
 ('group',['--weight-design','group_concentrated','--cases','iid_outputs']),
 ('low',['--weight-design','moderate','--memory','low','--cases','iid'])]:
 label='weighted-edges2-confirm-'+name
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','narrowedges2','weightededges2','--widths','6','--weight-kind','aw','--n','245760','--repeats','3','--trace']+args,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_EDGES2_CONFIRM_COMPLETE',flush=True)
