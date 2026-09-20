from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920'); python='target/one-fe-profile-20260919/python/bin/python'
cells=[]
for weight in ['pw','fw']:
 cells.append(['--label','weighted-edges2-'+weight,'--weight-kind',weight,'--weight-design','moderate','--cases','robust','cluster_cross','cluster2_saved','iid_outputs' if weight=='fw' else 'robust_saved'])
for design in ['constant','dispersed','group_concentrated']:
 cells.append(['--label','weighted-edges2-dist-'+design,'--weight-kind','aw','--weight-design',design,'--n','245760','--cases','iid_outputs'])
for threads in ['4','8']:
 cells.append(['--label','weighted-edges2-threads-'+threads,'--weight-kind','aw','--weight-design','moderate','--threads',threads,'--cases','iid'])
cells += [
 ['--label','weighted-edges2-low','--weight-kind','aw','--weight-design','moderate','--memory','low','--n','245760','--cases','iid'],
 ['--label','weighted-edges2-unit-control','--widths','5','--shuffled','--seed','31415927','--repeats','3','--cases','iid'],
 ['--label','weighted-edges2-onefe-control','--weight-kind','aw','--weight-design','moderate','--effects','g1','--cases','iid'],
 ['--label','weighted-edges2-bridge','--weight-kind','aw','--weight-design','bridge_cut','--design','weighted_bridge','--n','245760','--cases','iid','--record-reference-science-disagreement'],
]
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','narrowedges2','weightededges2','--widths','6','--repeats','1']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_EDGES2_VALIDATION_COMPLETE',flush=True)
