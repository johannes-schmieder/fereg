from pathlib import Path
import json,sys,csv,subprocess,shutil,statistics
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive,write,sha
from benchmarks.two_fe_dense.audit_outputs import audit
from benchmarks.two_fe_dense.graph_oracle import audit as graph_audit
root=Path('target/two-fe-broad-20260920');out=Path('benchmarks/reports/development/two-fe-useful-pilot-local-2026-09-20')
labels=sorted(p.name.removesuffix('-complete.json') for p in root.glob('useful-pilot-*-complete.json'))
archive(root,out,labels)
pairs=[];audits=set();graphs=[]
for label in labels:
 for p in sorted((root/'runs').glob(label+'-*-usefulpilot/result.json')):
  other=p.parent.with_name(p.parent.name.removesuffix('-usefulpilot')+'-fusedreconstruction')
  a=json.loads(p.read_text());b=json.loads((other/'result.json').read_text());cell=a['cell']
  def peak(d):return int(next(r['value'] for r in csv.DictReader((d/'fereg_runtime.csv').open()) if r['row']=='13' and r['column']=='2'))
  exact={n:sha(p.parent/(n+'.csv'))==sha(other/(n+'.csv')) for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
  assert exact['fereg_status'] and exact['fereg_dof'],(p,exact)
  rec=dict(run=p.parent.name,series=label,cell=cell,ratio=a['seconds']/b['seconds'],peak_candidate=peak(p.parent),peak_parent=peak(other),exact=exact)
  if (p.parent/'outputs.bin').exists():
   rec['outputs_exact']=sha(p.parent/'outputs.bin')==sha(other/'outputs.bin')
   assert cell['effects']==['g1','g2'],cell
   audits.add(label)
  def solver(d):return {(r['row'],r['column']):r['value'] for r in csv.DictReader((d/'fereg_solver.csv').open())}
  old,new=solver(other),solver(p.parent)
  rec['pilot_contraction_exact']=all(value==old[key] for key,value in new.items() if key[1]=='16')
  assert rec['pilot_contraction_exact'],p
  rec['initial_iterations']={'parent':old[('1','3')],'candidate':new[('1','3')]}
  pairs.append(rec)
  if cell['design']!='two_fe_dense' and cell['design']!='weighted_bridge' and cell['case']=='iid' and '-r1-' in p.parent.name and 'profile' not in label:
   graphs.append(p)
write(out/'native-pairs.json',pairs)
for label in sorted(audits):
 audit(root,label,'usefulpilot');shutil.copy(root/(label+'-usefulpilot-independent-outputs.json'),out)
graph_audit(graphs,out/'independent-graphs.json')
assert all(r['status']=='pass' for r in json.loads((out/'independent-graphs.json').read_text())['records'])
ident=json.loads((root/'identity-usefulpilot.json').read_text())
assert all(sha(Path(p))==h for p,h in ident['source'].items())
write(out/'source-audit.json',dict(source_files_verified=len(ident['source']),all_match=True,plugin_sha256=ident['plugin']))
(out/'implementation.patch').write_bytes(subprocess.check_output(['/opt/homebrew/bin/git','diff','--',*ident['overlays']]))
names=['check-usefulpilot.txt','check-usefulpilot-final.txt','test-usefulpilot.txt','test-usefulpilot-localize.txt','test-usefulpilot-final.txt']
for name in names:
 text=(root/name).read_text()
 text='\n'.join(line for line in text.splitlines() if not line.startswith(('   Compiling ', '    Checking ', '     Running ')))+'\n'
 (out/name).write_text(text.replace(str(Path.cwd()),'<repository>'))
write(out/'raw-test-hashes.json',{name:sha(root/name) for name in names})
for name in ['screen-usefulpilot.py','validate-usefulpilot.py','profile-usefulpilot.py','archive-usefulpilot.py']:
 shutil.copy(root/name,out)
print('USEFUL_PILOT_PAIRS',len(pairs),'exact',sum(all(r['exact'].values()) for r in pairs),'peak max delta',max(r['peak_candidate']-r['peak_parent'] for r in pairs),'peak increases',sum(r['peak_candidate']>r['peak_parent'] for r in pairs))
for s in json.loads((out/'summary.json').read_text()):
 chosen=[p for p in pairs if p['series']==s['series'] and p['cell']==s['cell']]
 print(s['series'],s['cell']['case'],s['cell']['k'],{k:round(v['seconds']*1000) for k,v in s['variants'].items()},'native ratio',statistics.median(p['ratio'] for p in chosen))
