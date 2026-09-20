from pathlib import Path
import subprocess,sys
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
sys.path.insert(0,str(Path.cwd()))
from benchmarks.stata_input_probe.fixtures import main
main(root,'weightedboth',['development/ols_weighted_edge_cache.do','development/ols_unit_edge_scratch.do','installation/estimate_ols_eclass_transaction.do'])
cells=[
 ['--label','weighted-both-shuffled','--widths','6','--cases','iid','iid_outputs','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-both-ordered','--widths','6','50','--cases','iid','iid_outputs','--weight-kind','aw','--weight-design','moderate','--repeats','3','--record-reference-output-disagreement'],
 ['--label','weighted-both-profile','--widths','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','1','--trace','--variants','unitboth','weightedboth'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','unitboth','weightedboth','fixest']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_BOTH_SCREEN_COMPLETE',flush=True)
