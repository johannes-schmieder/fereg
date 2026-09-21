from pathlib import Path
import json,csv,sys,shutil,statistics,math
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive,write,sha
from benchmarks.two_fe_dense.score import score
from benchmarks.two_fe_dense.audit_outputs import audit
r=Path('target/two-fe-broad-20260920');root=r/'onefe-controller-repair'
out=Path('benchmarks/reports/development/two-fe-broad-native-controls-local-2026-09-20');dest=out/'repair'
for name in ['identity-baseline.json','identity-usefulpilot.json','baseline-provenance.json']:
 shutil.copy(r/name,root/name)
progress=json.loads((root/'broad-onefe-repair-progress.json').read_text());assert len(progress)==12
labels=[p['label'] for p in progress]+['small-aw-confirm','small-aw-batch-diagnostic']
archive(root,dest,labels)
repair=score(root,'broad-onefe-repair');write(dest/'score.json',repair)
assert repair['identity_audit_complete'] and all(c['scientific_comparison']=='pass' for c in repair['cells'])
for suffix in ['plan','progress','complete']:
 shutil.copy(root/('broad-onefe-repair-'+suffix+'.json'),dest)
controller=dest/'controller';controller.mkdir()
for p in (root/'controller').iterdir():
 if p.is_file():shutil.copy(p,controller)
for m in json.loads((dest/'measurements.json').read_text()):
 assert all(sha(controller/n)==h for n,h in m['controller'].items())
shutil.copy(root/'controller.patch',dest)
independent=[];pairs=[]
for label in labels:
 for p in sorted((root/'runs').glob(label+'-*-usefulpilot/result.json')):
  other=p.parent.with_name(p.parent.name.removesuffix('-usefulpilot')+'-baseline')
  a=json.loads(p.read_text());b=json.loads((other/'result.json').read_text());assert a['status']==b['status']=='pass'
  def runtime(d):return {int(x['row']):float(x['value']) for x in csv.DictReader((d/'fereg_runtime.csv').open()) if x['column']=='2'}
  ar,br=runtime(p.parent),runtime(other)
  exact={n:sha(p.parent/(n+'.csv'))==sha(other/(n+'.csv')) for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
  assert exact['fereg_status'] and exact['fereg_dof']
  rec=dict(run=p.parent.name,label=label,ratio=a['seconds']/b['seconds'],candidate_seconds=a['seconds'],baseline_seconds=b['seconds'],exact=exact,
     peak_candidate=ar[13],peak_baseline=br[13],estimated_selected_candidate=ar[25],estimated_selected_baseline=br[25],modeled_ratio_candidate=ar[26],modeled_ratio_baseline=br[26],
     candidate_input_sha256=a['input_sha256'],baseline_input_sha256=b['input_sha256'],candidate_plugin_seconds=ar[21],baseline_plugin_seconds=br[21],
     candidate_native_seconds=ar[20],baseline_native_seconds=br[20],timed_command_repetitions=16 if 'batch' in label else 1)
  assert rec['candidate_input_sha256']==rec['baseline_input_sha256']
  if (p.parent/'outputs.bin').exists():rec['outputs_exact']=sha(p.parent/'outputs.bin')==sha(other/'outputs.bin')
  pairs.append(rec)
 if label.startswith('broad-onefe-repair'):
  audit(root,label,'usefulpilot')
  independent.extend(json.loads((root/(label+'-usefulpilot-independent-outputs.json')).read_text()))
write(dest/'native-pairs.json',pairs);write(dest/'independent-outputs.json',independent)
original=json.loads((out/'score.json').read_text())
by_cell={json.dumps(c['cell'],sort_keys=True):c for c in repair['cells']}
failures=[];cells=[]
for c in original['cells']:
 rec=dict(index=c['index'],cell=c['cell'],original_issues=c['issues'],original_median_ratio=c.get('median_paired_ratio'))
 if c['issues']:
  assert c['cell']['group']=='protected-one-fe' and c['cell']['case']=='cluster2_saved'
  fix=by_cell.pop(json.dumps(c['cell'],sort_keys=True));assert not fix['issues']
  rec.update(repair_label=f'broad-onefe-repair-c{fix["index"]:03d}',paired_ratio=fix['median_paired_ratio'],pairs=fix['pairs'],science=fix['scientific_comparison'])
  bad=list((r/'runs').glob(f'broad-native-controls-c{c["index"]:03d}-*-baseline/result.json'));assert len(bad)==1
  d=bad[0].parent;metadata=dict(line.split('\t',1) for line in (d/'metadata.tsv').read_text().splitlines())
  log=(d/'import.log').read_text();assert metadata['rc']=='0' and 'variable out_fe2 not found' in log and 'r(3598);' in log
  failed=json.loads(bad[0].read_text());assert failed['status']=='worker_failure'
  assert failed['input_sha256']==fix['pairs'][0]['input_sha256']
  failures.append(dict(index=c['index'],run=d.name,estimation_returncode=0,export_error='out_fe2 not found; r(3598)',result_sha256=sha(bad[0]),raw_log_sha256=sha(d/'import.log'),repaired_label=rec['repair_label']))
 else:rec.update(paired_ratio=c['median_paired_ratio'],pairs=c['pairs'],science=c['scientific_comparison'])
 cells.append(rec)
assert len(cells)==111 and len(failures)==12 and not by_cell
assert all(c['science']=='pass' for c in cells)
confirm=[p for p in pairs if p['label']=='small-aw-confirm'];batch=[p for p in pairs if p['label']=='small-aw-batch-diagnostic']
assert len(confirm)==len(batch)==5
c=cells[56];assert c['original_median_ratio']>1.05 and c['cell']['n']==7680 and c['cell']['weight_kind']=='aw'
c['confirmation_median_ratio']=statistics.median(p['ratio'] for p in confirm)
c['batch_diagnostic_median_ratio']=statistics.median(p['ratio'] for p in batch)
write(out/'export-failure-adjudication.json',failures)
write(out/'control-disposition.json',dict(planned=111,scientifically_compared=111,original_score_sha256=sha(out/'score.json'),repair_score_sha256=sha(dest/'score.json'),
  timing_threshold=1.05,timing_accepted=False,open_control_indices=[56],batch_diagnostic_replaces_gate=False,cells=cells))
allpairs=json.loads((out/'native-pairs.json').read_text())+pairs
memory=[]
for p in allpairs:
 if p['peak_candidate']>p['peak_baseline'] or p['modeled_ratio_candidate']>1.25:
  memory.append({k:p[k] for k in ['run','peak_candidate','peak_baseline','estimated_selected_candidate','estimated_selected_baseline','modeled_ratio_candidate','modeled_ratio_baseline']})
write(out/'memory-observations.json',memory)
for name in ['check-output-audit.txt','check-controller.txt','fixtures-usefulpilot-broad.json','broad-final-fixtures.py','review-repaired-controls.py']:
 shutil.copy(r/name,out)
write(out/'verification-log-hashes.json',{n:sha(r/n) for n in ['check-output-audit.txt','check-controller.txt','fixtures-usefulpilot-broad.stdout.txt']})
print('CONTROL_UNION',len(cells),'science passed; unresolved ratio',c['confirmation_median_ratio'],'batch diagnostic',c['batch_diagnostic_median_ratio'])
print('PAIRS',len(allpairs),'six exact',sum(all(p['exact'].values()) for p in allpairs),'output pairs',sum('outputs_exact' in p for p in allpairs),'outputs exact',sum(p.get('outputs_exact',False) for p in allpairs),'independent',len(json.loads((out/'independent-outputs.json').read_text()))+len(independent))
for group in sorted({c['cell']['group'] for c in cells}):
 vals=[c['paired_ratio'] for c in cells if c['cell']['group']==group]
 print('GROUP',group,len(vals),math.exp(statistics.mean(math.log(x) for x in vals)),max(vals))
print('REPAIR_WORST',repair['worst_ratio'])
print('MEMORY_INCREASES',sum(p['peak_candidate']>p['peak_baseline'] for p in allpairs),'max delta',max(p['peak_candidate']-p['peak_baseline'] for p in allpairs),'early ratio max',max(p['modeled_ratio_candidate'] for p in allpairs))
assert all(p['modeled_ratio_candidate']==p['modeled_ratio_baseline'] and p['peak_candidate']==p['peak_baseline'] for p in allpairs if p['modeled_ratio_candidate']>1.25)
print('ALL_EARLY_RATIO_EXCEEDANCES_UNCHANGED_FROM_BASELINE')
