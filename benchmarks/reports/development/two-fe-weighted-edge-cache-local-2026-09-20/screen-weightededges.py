from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[
 ['--label','weighted-edges2-shuffled','--variants','narrowedges2','weightededges2','fixest','--weight-kind','aw','--weight-design','moderate','--widths','6','--cases','iid','iid_outputs','--shuffled','--seed','31415927','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-edges2-ordered','--variants','narrowedges2','weightededges2','fixest','--weight-kind','aw','--weight-design','moderate','--widths','6','50','--cases','iid','iid_outputs','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-edges2-profile','--variants','narrowedges2','weightededges2','--weight-kind','aw','--weight-design','moderate','--widths','6','--cases','iid','--shuffled','--seed','31415927','--repeats','1','--trace'],
 ['--label','weighted-edges2-strong6-aw','--variants','narrowedges2','weightededges2','fixest','--weight-kind','aw','--weight-design','moderate','--design','strong_d6','--n','245760','--widths','6','--cases','iid','--repeats','3','--trace'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root)]+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_EDGES2_SCREEN_COMPLETE',flush=True)
