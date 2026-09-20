from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','narrow-input3-unit-ordered','--widths','1','3','5','6','--cases','iid','--repeats','3'],
 ['--label','narrow-input3-unit-shuffled','--widths','1','3','5','6','--cases','iid','--shuffled','--seed','31415927','--repeats','3'],
 ['--label','narrow-input3-aw','--widths','1','3','5','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--repeats','3'],
 ['--label','narrow-input3-onefe','--widths','1','5','--cases','iid','--effects','g1','--repeats','3'],
 ['--label','narrow-input3-profile','--widths','5','--cases','iid','--shuffled','--seed','31415927','--repeats','1','--trace','--variants','weightededges2','narrowinput3'],
 ['--label','narrow-input3-control-wide','--widths','50','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput3'],
 ['--label','narrow-input3-control-threads','--widths','1','--threads','4','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput3'],
 ['--label','narrow-input3-control-low','--widths','1','--memory','low','--n','245760','--cases','iid','--repeats','1','--variants','weightededges2','narrowinput3'],
]
cells.append(['--label','narrow-input3-aw-shuffled','--widths','6','--cases','iid','iid_outputs','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','3','--record-reference-output-disagreement'])
for design in ['strong_d3','strong_d6','unbalanced']:
 for weight in ['none','aw']:
  cells.append(['--label',f'narrow-input3-{design}-{weight}','--design',design,'--n','245760','--widths','6','--weight-kind',weight,'--weight-design','none' if weight=='none' else 'moderate','--cases','iid','--repeats','3'])
cells.append(['--label','narrow-input3-aw-profile','--widths','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','1','--trace','--variants','weightededges2','narrowinput3'])
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightededges2','narrowinput3','fixest']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('NARROW_INPUT3_SCREEN_COMPLETE',flush=True)
