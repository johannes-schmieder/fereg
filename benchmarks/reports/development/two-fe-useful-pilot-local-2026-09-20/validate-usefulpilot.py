from pathlib import Path
import subprocess
root=Path('target/two-fe-broad-20260920');python='target/one-fe-profile-20260919/python/bin/python'
cells=[]
for weight in ['pw','fw']:
 cells.append(['--label','useful-pilot-control-'+weight,'--weight-kind',weight,'--weight-design','moderate','--cases','robust','cluster_cross','cluster2_saved'])
for design in ['constant','dispersed','group_concentrated']:
 cells.append(['--label','useful-pilot-control-'+design,'--weight-kind','aw','--weight-design',design,'--n','245760','--cases','iid_outputs'])
for design in ['strong_d2','two_fe_sparse','weak_d3','bottleneck_ring','cycle','disconnected_blocks']:
 for weight in ['none','aw']:
  cells.append(['--label','useful-pilot-control-'+design+'-'+weight,'--design',design,'--weight-kind',weight,'--weight-design','none' if weight=='none' else 'moderate','--n','245760','--cases','iid'])
cells += [
 ['--label','useful-pilot-control-bridge','--weight-kind','aw','--weight-design','bridge_cut','--design','weighted_bridge','--n','245760','--cases','iid'],
 ['--label','useful-pilot-control-low','--weight-kind','aw','--weight-design','moderate','--memory','low','--n','245760','--cases','iid'],
 ['--label','useful-pilot-control-t4','--weight-kind','aw','--weight-design','moderate','--threads','4','--cases','iid'],
 ['--label','useful-pilot-control-t8','--weight-kind','aw','--weight-design','moderate','--threads','8','--cases','iid'],
 ['--label','useful-pilot-control-onefe','--effects','g1','--weight-kind','aw','--weight-design','moderate','--widths','1','6','50','--cases','robust'],
 ['--label','useful-pilot-control-unitwide','--widths','50','--cases','iid','--repeats','3'],
]
for cell in cells:
 with (root/(cell[1]+'.stdout.txt')).open('w') as out:
  code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--variants','fusedreconstruction','usefulpilot','--widths','6','--repeats','1']+cell,stdout=out,stderr=subprocess.STDOUT)
 print(cell[1],code,flush=True)
 if code:raise SystemExit(code)
print('USEFUL_PILOT_VALIDATION_COMPLETE',flush=True)
