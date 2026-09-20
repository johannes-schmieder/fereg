from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','weighted-statistics-main-shuffled','--widths','6','--cases','iid','iid_outputs','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-statistics-main-ordered','--widths','6','50','--cases','iid','iid_outputs','--weight-kind','aw','--weight-design','moderate','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-statistics-main-onefe','--widths','1','6','50','--cases','iid','--weight-kind','aw','--weight-design','moderate','--effects','g1','--repeats','3'],
]
for graph in ['strong_d3','strong_d6','unbalanced']:
 cells.append(['--label','weighted-statistics-main-'+graph,'--design',graph,'--n','245760','--widths','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--repeats','3'])
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightedboth','weightedstatistics','fixest']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_STATISTICS_MAIN_SCREEN_COMPLETE',flush=True)
