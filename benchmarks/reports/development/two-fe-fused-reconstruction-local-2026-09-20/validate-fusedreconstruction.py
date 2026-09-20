from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[]
for weight in ['pw','fw']:
 cells.append(['--label','fused-reconstruction-control-'+weight,'--weight-kind',weight,'--weight-design','moderate','--cases','robust','cluster_cross','cluster2_saved'])
for design in ['dispersed','group_concentrated']:
 cells.append(['--label','fused-reconstruction-control-'+design,'--weight-kind','aw','--weight-design',design,'--n','245760','--cases','iid_outputs'])
cells += [
 ['--label','fused-reconstruction-control-strong6','--weight-kind','aw','--weight-design','moderate','--design','strong_d6','--n','245760','--cases','iid','--repeats','3'],
 ['--label','fused-reconstruction-control-bridge','--weight-kind','aw','--weight-design','bridge_cut','--design','weighted_bridge','--n','245760','--cases','iid'],
 ['--label','fused-reconstruction-control-low','--weight-kind','aw','--weight-design','moderate','--memory','low','--n','245760','--cases','iid'],
 ['--label','fused-reconstruction-control-t4','--weight-kind','aw','--weight-design','moderate','--threads','4','--cases','iid'],
 ['--label','fused-reconstruction-control-onefe','--effects','g1','--weight-kind','aw','--weight-design','moderate','--widths','1','6','50','--cases','robust'],
 ['--label','fused-reconstruction-control-unitwide','--widths','50','--cases','iid','--repeats','3'],
]
for cell in cells:
 with (root/(cell[1]+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','weightedstatistics','fusedreconstruction','--widths','6','--repeats','1']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(cell[1],code,flush=True)
 if code:raise SystemExit(code)
print('FUSED_RECONSTRUCTION_VALIDATION_COMPLETE',flush=True)
