"""Join local development evidence; this is not release qualification."""
from pathlib import Path
import json, shutil, sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import sha, write

reports=Path('benchmarks/reports/development')
out=reports/'two-fe-broad-primary-local-2026-09-20'
sources={
    'primary':out/'primary-audit.json',
    'holdouts':reports/'two-fe-fresh-holdouts-local-2026-09-20/holdout-audit.json',
    'native_controls':reports/'two-fe-broad-native-controls-local-2026-09-20/control-disposition.json',
    'small_precision':reports/'two-fe-single-command-clock-local-2026-09-20/precision-audit.json',
    'reference_accuracy':reports/'two-fe-final-reference-accuracy-local-2026-09-20/accuracy-audit.json',
    'memory_followup':reports/'two-fe-memory-followup-local-2026-09-20/memory-audit.json',
}
r={n:json.loads(p.read_text()) for n,p in sources.items()}
assert r['primary']['cells']==77 and r['primary']['timing_threshold_met']
assert r['primary']['native_graph_oracles']==360 and r['primary']['independent_output_audits']==80
assert r['holdouts']['planned_cells']==16 and r['holdouts']['timing_acceptance']
assert r['holdouts']['native_scientific_oracles']==18 and r['holdouts']['independent_output_audits']==20
controls=r['native_controls']['cells'];assert len(controls)==111
assert all(c['science']=='pass' for c in controls)
assert all(c['paired_ratio']<=1.05 for c in controls if c['index']!=56)
assert r['small_precision']['status']=='pass' and len(r['small_precision']['blocks'])==2
assert all(b['highres_median_ratio']<=1.05 for b in r['small_precision']['blocks'])
assert r['reference_accuracy']['distinct_cells']==28 and r['reference_accuracy']['successful_native_calls']==28
assert r['reference_accuracy']['all_native_science_identical_to_headline']
assert r['reference_accuracy']['all_requested_native_outputs_identical_to_headline']
assert r['reference_accuracy']['reference_identity']==r['primary']['reference_identity']
assert r['memory_followup']['cells']==21 and r['memory_followup']['peak_deltas_match_prior']
assert r['memory_followup']['max_observed_over_whole_action_forecast']<=1.25
assert r['memory_followup']['complete_memory_proof'] is False
base=Path('target/two-fe-broad-20260920')
identity=json.loads((base/'identity-usefulpilot.json').read_text())
assert len(identity['source'])==516 and all(sha(Path(p))==h for p,h in identity['source'].items())
assert identity['plugin']==r['primary']['candidate_plugin']==sha(base/'usefulpilot/dist/fereg_macos.plugin')
for folder in ['two-fe-fresh-holdouts-local-2026-09-20','two-fe-broad-native-controls-local-2026-09-20',
               'two-fe-single-command-clock-local-2026-09-20','two-fe-memory-followup-local-2026-09-20',
               'two-fe-final-reference-accuracy-local-2026-09-20']:
    identities=json.loads((reports/folder/'identities.json').read_text())
    assert identities['usefulpilot']==identity
fixtures=[]
for p in [reports/'two-fe-useful-pilot-local-2026-09-20/fixtures-usefulpilot.json',
          reports/'two-fe-broad-native-controls-local-2026-09-20/fixtures-usefulpilot-broad.json']:
    checks=json.loads(p.read_text());assert all(c['status']=='pass' and c['process_returncode']==0 and c['terminal_sentinel'] for c in checks)
    fixtures.append(dict(path=str(p),sha256=sha(p),checks=len(checks)))
target=out/'development-acceptance.json';assert not target.exists()
write(target,dict(schema='broad-two-way-ols-local-development-acceptance-v1',status='complete',
    scope='Predeclared local Mac ordinary categorical two-FE OLS milestone',release_qualification=False,
    universal_performance_claim=False,source_files_verified=516,candidate_plugin=identity['plugin'],
    primary_cells=77,holdout_cells=16,native_control_cells=111,
    timing_blocks=2,pairs_per_cell_per_block=5,all_primary_and_holdout_cell_medians_below_one=True,
    source_evidence={n:dict(path=str(p),sha256=sha(p)) for n,p in sources.items()},fixtures=fixtures,
    limitations=[
        'Default reference output/science mismatches remain; separate tighter accuracy calls include explicit nonconvergence.',
        'Frequency weights use native baseline and independent literal expansion; ordinary fixest weights are not a frequency-inference comparator.',
        'The aspirational ten-percent margin is not reached in every cell.',
        'Seven native control peaks increased within existing forecasts; strict memory proof gaps remain.',
        'Earlier failed timer checks, exporter attempts and Stata startup crash remain archived.',
        'The result is local development evidence, not universal, platform or release qualification.'
    ]))
shutil.copy(Path(__file__),out)
print('BROAD_TWO_WAY_LOCAL_DEVELOPMENT_ACCEPTED',flush=True)
