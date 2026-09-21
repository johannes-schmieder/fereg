from pathlib import Path
import csv,json,sys,hashlib,shutil,statistics
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive,write,sha
from benchmarks.two_fe_dense.score import score
from benchmarks.two_fe_dense.audit_outputs import audit
root=Path('target/two-fe-broad-20260920');label='broad-native-controls'
out=Path('benchmarks/reports/development/two-fe-broad-native-controls-local-2026-09-20')
plan=json.loads((root/(label+'-plan.json')).read_text())
progress=json.loads((root/(label+'-progress.json')).read_text())
assert len(progress)==len(plan['cells'])==111
labels=[p['label'] for p in progress]
archive(root,out,labels)
controller=out/'original-controller';controller.mkdir()
for name in ['common.py','measure.py','stata_worker.do','r_worker.R']:
 shutil.copy(root/'controller'/name,controller)
for r in json.loads((out/'measurements.json').read_text()):
 assert all(sha(controller/name)==value for name,value in r['controller'].items())
write(out/'original-controller-hashes.json',{p.name:sha(p) for p in controller.iterdir()})
scored=score(root,label);write(out/'score.json',scored)
for suffix in ['plan','progress','complete']:
 shutil.copy(root/(label+'-'+suffix+'.json'),out)
pairs=[];independent=[]
for attempt in progress:
 for p in sorted((root/'runs').glob(attempt['label']+'-*-usefulpilot/result.json')):
  other=p.parent.with_name(p.parent.name.removesuffix('-usefulpilot')+'-baseline')
  if not (other/'result.json').exists():continue
  a=json.loads(p.read_text());b=json.loads((other/'result.json').read_text())
  if a['status']!='pass' or b['status']!='pass':continue
  def runtime(d):return {int(r['row']):float(r['value']) for r in csv.DictReader((d/'fereg_runtime.csv').open()) if r['column']=='2'}
  ar,br=runtime(p.parent),runtime(other)
  exact={n:sha(p.parent/(n+'.csv'))==sha(other/(n+'.csv')) for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
  rec=dict(run=p.parent.name,index=attempt['index'],cell=attempt['cell'],ratio=a['seconds']/b['seconds'],exact=exact,
    peak_candidate=ar[13],peak_baseline=br[13],estimated_selected_candidate=ar[25],estimated_selected_baseline=br[25],modeled_ratio_candidate=ar[26],modeled_ratio_baseline=br[26])
  if (p.parent/'outputs.bin').exists():rec['outputs_exact']=sha(p.parent/'outputs.bin')==sha(other/'outputs.bin')
  pairs.append(rec)
 if attempt['comparison_status']=='pass' and attempt['cell']['case'] in ['iid_outputs','iid_saved','cluster_fe_saved','cluster2_saved','robust_saved']:
  audit(root,attempt['label'],'usefulpilot')
  path=root/(attempt['label']+'-usefulpilot-independent-outputs.json')
  independent.extend(json.loads(path.read_text()))
write(out/'native-pairs.json',pairs);write(out/'independent-outputs.json',independent)
ident=json.loads((root/'identity-usefulpilot.json').read_text())
assert all(sha(Path(p))==h for p,h in ident['source'].items())
write(out/'source-audit.json',dict(all_match=True,source_files_verified=len(ident['source']),plugin_sha256=ident['plugin']))
shutil.copy(root/'review-native-controls.py',out)
print('NATIVE_CONTROLS_SCORE',scored['completely_measured'],'/',scored['planned'],'worst',scored['worst_ratio'],'threshold',scored['timing_threshold_met'])
print('PAIRS',len(pairs),'six_exact',sum(all(p['exact'].values()) for p in pairs),'outputs',sum('outputs_exact' in p for p in pairs),'exact_outputs',sum(p.get('outputs_exact',False) for p in pairs),'independent_outputs',len(independent))
print('PEAK_INCREASES',sum(p['peak_candidate']>p['peak_baseline'] for p in pairs),'max_delta',max(p['peak_candidate']-p['peak_baseline'] for p in pairs),'model_above_125',sum(p['modeled_ratio_candidate']>1.25 for p in pairs))
for cell in scored['cells']:
 if cell['issues'] or cell.get('median_paired_ratio',0)>1.05 or cell['scientific_comparison']!='pass':print('REVIEW',cell['index'],cell['cell'],cell.get('median_paired_ratio'),cell['issues'],cell['scientific_comparison'])
