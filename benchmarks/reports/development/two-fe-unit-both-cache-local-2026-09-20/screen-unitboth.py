from pathlib import Path
import subprocess,sys
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
sys.path.insert(0,str(Path.cwd()))
from benchmarks.stata_input_probe.fixtures import main
main(root,'unitboth',['development/ols_unit_edge_scratch.do','development/ols_weighted_edge_cache.do','installation/estimate_ols_eclass_transaction.do'])
cells=[
 ['--label','unit-both-ordered','--widths','1','3','5','6','--cases','iid','--repeats','3'],
 ['--label','unit-both-shuffled','--widths','1','3','5','6','--cases','iid','--shuffled','--seed','31415927','--repeats','3'],
 ['--label','unit-both-outputs','--widths','3','5','--cases','iid_outputs','cluster2_saved','--shuffled','--seed','31415927','--repeats','1','--record-reference-output-disagreement'],
 ['--label','unit-both-strong6','--design','strong_d6','--n','245760','--widths','6','--cases','iid','--repeats','3'],
 ['--label','unit-both-wide','--widths','50','--cases','iid','--repeats','1'],
 ['--label','unit-both-aw','--widths','6','--cases','iid','--weight-kind','aw','--weight-design','moderate','--shuffled','--seed','31415927','--repeats','1'],
 ['--label','unit-both-profile','--widths','5','--cases','iid','--shuffled','--seed','31415927','--repeats','1','--trace','--variants','narrowinput3','unitboth'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','narrowinput3','unitboth','fixest']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('UNIT_BOTH_SCREEN_COMPLETE',flush=True)
