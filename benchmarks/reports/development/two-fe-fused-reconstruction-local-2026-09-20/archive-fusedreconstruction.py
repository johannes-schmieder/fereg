from pathlib import Path
import json,sys,csv,subprocess,shutil,statistics
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive,write,sha
from benchmarks.two_fe_dense.audit_outputs import audit
root=Path('target/two-fe-broad-20260920');out=Path('benchmarks/reports/development/two-fe-fused-reconstruction-local-2026-09-20')
labels=sorted(p.name.removesuffix('-complete.json') for p in root.glob('fused-reconstruction-*-complete.json'))
archive(root,out,labels)
pairs=[];audits=set()
for label in labels:
 for p in sorted((root/'runs').glob(label+'-*-fusedreconstruction/result.json')):
  other=p.parent.with_name(p.parent.name.removesuffix('-fusedreconstruction')+'-weightedstatistics')
  a=json.loads(p.read_text());b=json.loads((other/'result.json').read_text());cell=a['cell']
  def peak(d):return int(next(r['value'] for r in csv.DictReader((d/'fereg_runtime.csv').open()) if r['row']=='13' and r['column']=='2'))
  exact={n:sha(p.parent/(n+'.csv'))==sha(other/(n+'.csv')) for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
  assert all(exact.values()),(p,exact)
  rec=dict(run=p.parent.name,series=label,cell=cell,ratio=a['seconds']/b['seconds'],peak_candidate=peak(p.parent),peak_parent=peak(other),exact=exact)
  if (p.parent/'outputs.bin').exists():
   rec['outputs_exact']=sha(p.parent/'outputs.bin')==sha(other/'outputs.bin')
   assert rec['outputs_exact']
   assert cell['effects']==['g1','g2'],cell
   audits.add(label)
  pairs.append(rec)
write(out/'native-pairs.json',pairs)
phases=[]
for p in sorted((root/'runs').glob('fused-reconstruction-profile-*/stderr.txt')):
 selected=[];following_route=False
 for line in p.read_text().splitlines():
  if line.startswith('FEREG_PHASE|schur_route|'):following_route=True
  elif following_route and line.startswith('FEREG_PHASE|schur|'):
   selected.append(line);following_route=False
 assert len(selected)==1,(p,selected)
 phases.append(dict(run=p.parent.name,measured_initial_schur=selected[0]))
write(out/'reconstruction-phases.json',phases)

for label in sorted(audits):
 audit(root,label,'fusedreconstruction');shutil.copy(root/(label+'-fusedreconstruction-independent-outputs.json'),out)
ident=json.loads((root/'identity-fusedreconstruction.json').read_text())
assert all(sha(Path(p))==h for p,h in ident['source'].items())
write(out/'source-audit.json',dict(source_files_verified=len(ident['source']),all_match=True,plugin_sha256=ident['plugin']))
(out/'implementation.patch').write_bytes(subprocess.check_output(['/opt/homebrew/bin/git','diff','--',*ident['overlays']]))
name='test-fusedreconstruction.txt';text=(root/name).read_text()
(out/name).write_text('\n'.join(line for line in text.splitlines() if line.startswith(('test ','thread ','running ')))+'\n')
write(out/'raw-test-hashes.json',{name:sha(root/name)})
for name in ['screen-fusedreconstruction.py','validate-fusedreconstruction.py','profile-fusedreconstruction.py']:
 shutil.copy(root/name,out)
print('FUSED_RECONSTRUCTION_PAIRS',len(pairs),'exact',sum(all(r['exact'].values()) for r in pairs),'peak max delta',max(r['peak_candidate']-r['peak_parent'] for r in pairs),'peak increases',sum(r['peak_candidate']>r['peak_parent'] for r in pairs))
for s in json.loads((out/'summary.json').read_text()):
 chosen=[p for p in pairs if p['series']==s['series'] and p['cell']==s['cell']]
 print(s['series'],s['cell']['case'],s['cell']['k'],{k:round(v['seconds']*1000) for k,v in s['variants'].items()},'native ratio',statistics.median(p['ratio'] for p in chosen))
