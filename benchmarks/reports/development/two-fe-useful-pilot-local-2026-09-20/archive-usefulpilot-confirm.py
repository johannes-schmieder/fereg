from pathlib import Path
import json,sys,csv,shutil,statistics
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive,write,sha
from benchmarks.two_fe_dense.audit_outputs import audit
root=Path('target/two-fe-broad-20260920');out=Path('benchmarks/reports/development/two-fe-useful-pilot-local-2026-09-20')
labels=['useful-pilot-confirm-'+x for x in ['dispersed','low','sparse-aw']]
archive(root,out/'confirmation',labels)
pairs=[]
for label in labels:
 for p in sorted((root/'runs').glob(label+'-*-usefulpilot/result.json')):
  other=p.parent.with_name(p.parent.name.removesuffix('-usefulpilot')+'-fusedreconstruction')
  a=json.loads(p.read_text());b=json.loads((other/'result.json').read_text())
  def peak(d):return int(next(r['value'] for r in csv.DictReader((d/'fereg_runtime.csv').open()) if r['row']=='13' and r['column']=='2'))
  exact={n:sha(p.parent/(n+'.csv'))==sha(other/(n+'.csv')) for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
  assert all(exact.values()),(p,exact)
  rec=dict(run=p.parent.name,series=label,ratio=a['seconds']/b['seconds'],peak_candidate=peak(p.parent),peak_parent=peak(other),exact=exact)
  if (p.parent/'outputs.bin').exists():
   rec['outputs_exact']=sha(p.parent/'outputs.bin')==sha(other/'outputs.bin');assert rec['outputs_exact']
  pairs.append(rec)
write(out/'confirmation/native-pairs.json',pairs)
audit(root,'useful-pilot-confirm-dispersed','usefulpilot')
shutil.copy(root/'useful-pilot-confirm-dispersed-usefulpilot-independent-outputs.json',out/'confirmation')
for name in ['confirm-usefulpilot.py','archive-usefulpilot-confirm.py']:
 shutil.copy(root/name,out)
print('CONFIRMATION_PAIRS',len(pairs),'peak max delta',max(r['peak_candidate']-r['peak_parent'] for r in pairs))
for label in labels:print(label,statistics.median(r['ratio'] for r in pairs if r['series']==label))
phases=[]
for p in sorted((root/'runs').glob('useful-pilot-profile-*/stderr.txt')):
 lines=p.read_text().splitlines();route=next(i for i,l in enumerate(lines) if l.startswith('FEREG_PHASE|schur_route|'))
 initial=next(l for l in lines[route+1:] if l.startswith('FEREG_PHASE|schur|'))
 solver={(r['row'],r['column']):float(r['value']) for r in csv.DictReader((p.parent/'fereg_solver.csv').open())}
 phases.append(dict(run=p.parent.name,route=lines[route],initial=initial,warm_pilot=next((l for l in lines[route+1:] if l.startswith('FEREG_PHASE|schur_warm_pilot|')),None),initial_report_iterations=int(solver[('1','3')]),initial_projection_seconds=solver[('1','9')]+solver[('1','10')]))
write(out/'pilot-phases.json',phases)
for row in phases:print(row['run'],row['initial_report_iterations'],round(row['initial_projection_seconds']*1000,3))
