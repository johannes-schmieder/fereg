from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920'); python='target/one-fe-profile-20260919/python/bin/python'
cells=[]
for weight in ['pw','fw']:
 cells.append(['--label','weighted-statistics-control-'+weight,'--weight-kind',weight,'--weight-design','moderate','--cases','robust','cluster_cross','cluster2_saved','iid_outputs' if weight=='fw' else 'robust_saved'])
for design in ['constant','dispersed','group_concentrated']:
 cells.append(['--label','weighted-statistics-control-dist-'+design,'--weight-kind','aw','--weight-design',design,'--n','245760','--cases','iid_outputs'])
for threads in ['4','8']:
 cells.append(['--label','weighted-statistics-control-threads-'+threads,'--weight-kind','aw','--weight-design','moderate','--threads',threads,'--cases','iid'])
cells += [
 ['--label','weighted-statistics-control-low','--weight-kind','aw','--weight-design','moderate','--memory','low','--n','245760','--cases','iid'],
 ['--label','weighted-statistics-control-unit-control','--widths','5','--shuffled','--seed','31415927','--repeats','3','--cases','iid'],
 ['--label','weighted-statistics-control-bridge','--weight-kind','aw','--weight-design','bridge_cut','--design','weighted_bridge','--n','245760','--cases','iid','--record-reference-science-disagreement'],
]
for n in ['16383','16384']:
 cells.append(['--label','weighted-statistics-control-n'+n,'--weight-kind','aw','--weight-design','moderate','--n',n,'--cases','iid'])
for weight in ['pw','fw']:
 cells.append(['--label','weighted-statistics-control-onefe-'+weight,'--weight-kind',weight,'--weight-design','moderate','--effects','g1','--widths','1','6','50','--cases','robust'])
cells.append(['--label','weighted-statistics-control-unit-wide','--widths','50','--cases','iid','--repeats','3'])
for cell in cells:
 label=cell[1]
 with (root/(label+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightedboth','weightedstatistics','--widths','6','--repeats','1']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(label,code,flush=True)
 if code:raise SystemExit(code)
print('WEIGHTED_STATISTICS_VALIDATION_COMPLETE',flush=True)
