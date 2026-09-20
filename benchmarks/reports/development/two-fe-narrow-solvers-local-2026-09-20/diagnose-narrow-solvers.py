from pathlib import Path
import subprocess,json,sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.run import profile
root=Path('target/two-fe-broad-20260920'); python='target/one-fe-profile-20260919/python/bin/python'
checks=[]
for design in ['strong_d3','strong_d6','unbalanced']:
 for weight in ['none','aw']:
  reference=None
  for solver in ['auto','map','cg','lsmr','cmg']:
   label=f'narrow-solver-{design}-{weight}-{solver}'
   with (root/(label+'.stdout.txt')).open('w') as out:
    code=subprocess.call([python,'benchmarks/two_fe_dense/run.py',str(root),'--label',label,'--variants','weightededges2','--widths','6','--n','245760','--weight-kind',weight,'--weight-design','none' if weight=='none' else 'moderate','--design',design,'--solver',solver,'--repeats','1','--trace'],stdout=out,stderr=subprocess.STDOUT)
   results=list((root/'runs').glob(label+'-*/result.json'))
   check=dict(design=design,weight=weight,solver=solver,process_returncode=code,results=[p.parent.name for p in results])
   if code==0:
    assert len(results)==1
    result=json.loads(results[0].read_text());check['seconds']=result['seconds']
    if reference is None:reference=results[0].parent
    try:check['science']=profile.compare(reference,results[0].parent,exact_native=False)
    except AssertionError as e:check['science']=dict(status='mismatch',detail=str(e))
   checks.append(check)
   (root/'narrow-solver-diagnostics.json').write_text(json.dumps(checks,indent=2)+'\n')
   print(label,code,check.get('seconds'),flush=True)
print('NARROW_SOLVER_DIAGNOSTICS_COMPLETE',flush=True)
