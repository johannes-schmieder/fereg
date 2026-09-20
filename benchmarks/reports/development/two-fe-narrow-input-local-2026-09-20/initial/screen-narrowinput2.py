from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','narrow-input2-unit-ordered','--widths','1','3','5','--cases','iid','--repeats','3'],
 ['--label','narrow-input2-unit-shuffled','--widths','1','3','5','--cases','iid','--shuffled','--seed','31415927','--repeats','3'],
 ['--label','narrow-input2-aw','--widths','1','3','5','--cases','iid','--weight-kind','aw','--weight-design','moderate','--repeats','3'],
 ['--label','narrow-input2-onefe','--widths','1','5','--cases','iid','--effects','g1','--repeats','3'],
 ['--label','narrow-input2-profile','--widths','5','--cases','iid','--shuffled','--seed','31415927','--repeats','1','--trace','--variants','weightededges2','narrowinput2'],
 ['--label','narrow-input2-control-wide','--widths','50','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput2'],
 ['--label','narrow-input2-control-threads','--widths','1','--threads','4','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput2'],
 ['--label','narrow-input2-control-low','--widths','1','--memory','low','--n','245760','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput2'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightededges2','narrowinput2','fixest']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('NARROW_INPUT_SCREEN_COMPLETE',flush=True)
