from pathlib import Path
import json,hashlib,subprocess,tarfile,io
r=Path('target/two-fe-broad-20260920');out=Path('benchmarks/reports/development/two-fe-broad-native-controls-local-2026-09-20')
read=lambda p:json.loads(p.read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=read(out/'score.json');repair=read(out/'repair/score.json');disposition=read(out/'control-disposition.json')
assert repair['timing_threshold_met'] and repair['planned']==12
assert len(disposition['cells'])==111
assert disposition['open_control_indices']==[c['index'] for c in disposition['cells'] if c['paired_ratio']>1.05]==[56]
assert disposition['cells'][56]['confirmation_median_ratio']==1.0625
assert not disposition['timing_accepted'] and not disposition['batch_diagnostic_replaces_gate']
assert all(len(c['pairs'])==3 and c['science']=='pass' for c in disposition['cells'])
measurements=read(out/'measurements.json')+read(out/'repair/measurements.json')
assert len(measurements)==698 and sum(x['status']=='pass' for x in measurements)==686
assert len([m for m in measurements if m['series']=='small-aw-batch-diagnostic'])==10
assert len(read(out/'export-failure-adjudication.json'))==12
pairs=read(out/'native-pairs.json')+read(out/'repair/native-pairs.json')
assert len(pairs)==343 and sum('outputs_exact' in p for p in pairs)==129
assert all(p['peak_candidate']<p['estimated_selected_candidate'] for p in pairs if p['peak_candidate']>p['peak_baseline'])
assert all(p['peak_candidate']==p['peak_baseline'] and p['modeled_ratio_candidate']==p['modeled_ratio_baseline'] for p in pairs if p['modeled_ratio_candidate']>1.25)
base=read(r/'identity-baseline.json');current=read(r/'identity-usefulpilot.json')
archive=subprocess.check_output(['/opt/homebrew/bin/git','archive','e9d593bdff116373fd09496d008be71d01b0a726','--',*base['source']])
with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
 hashes={m.name:hashlib.sha256(tar.extractfile(m).read()).hexdigest() for m in tar if m.isfile()}
assert all(hashes[p]==h for p,h in base['source'].items())
assert all(sha(Path(p))==h for p,h in current['source'].items())
for name,identity in [('baseline',base),('usefulpilot',current)]:
 assert identity['abi']=='pass' and sha(r/name/'dist/fereg_macos.plugin')==identity['plugin']
result=dict(status='pass',standard_workers=688,diagnostic_workers=10,preserved_export_failures=12,scientific_inventory_cells=111,native_pairs_including_confirmation_and_batch=343,
 independent_output_pairs=129,open_timing_control=56,timing_accepted=False,baseline_commit_source_files_verified=len(base['source']),candidate_source_files_verified=len(current['source']))
(out/'final-evidence-check.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
print('FINAL_CONTROL_EVIDENCE_CHECK_PASS',result)
