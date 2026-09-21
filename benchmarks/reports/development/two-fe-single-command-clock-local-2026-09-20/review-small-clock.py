from pathlib import Path
import csv, json, statistics, shutil, sys
sys.path.insert(0, str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive, write, sha

base = Path('target/two-fe-broad-20260920')
root = base / 'small-clock-v3'
output = Path('benchmarks/reports/development/two-fe-single-command-clock-local-2026-09-20')
plan = json.loads((root / 'precision-plan.json').read_text())
labels = plan['labels']
assert plan['pairs_per_block'] == 5 and plan['timed_commands'] == 1
identity = json.loads((root / 'controller/clock-identity.json').read_text())
assert sha(root / 'clock/clock.plugin') == identity['plugin_sha256']
assert all(sha(Path(p)) == h for p, h in identity['sources'].items())
fixture = json.loads((root / 'clock/fixture.json').read_text())
assert fixture['status'] == 'pass'
pairs, blocks = [], []
for label in labels:
    complete = json.loads((root / (label + '-complete.json')).read_text())
    assert complete['status'] == 'pass' and len(complete['checks']) == 5
    for rep in range(1, 6):
        prefix = f'{label}-iid-n7680-k6-s8675309-shuffle0-t16-r{rep}-'
        directories = [root / 'runs' / (prefix + variant) for variant in ['baseline', 'usefulpilot']]
        b, a = [json.loads((p / 'result.json').read_text()) for p in directories]
        assert all(r['status'] == 'pass' and r['sentinel'] and r['process_returncode'] == 0 for r in [a,b])
        assert a['cell'] == b['cell'] and a['sample_sha256'] == b['sample_sha256']
        assert a['input_sha256'] == b['input_sha256']
        assert a['metadata']['command'] == b['metadata']['command']
        original = json.loads((base / 'onefe-controller-repair/runs' /
            f'small-aw-confirm-iid-n7680-k6-s8675309-shuffle0-t16-r{rep}-usefulpilot/result.json').read_text())
        assert a['cell'] == original['cell'] and a['input_sha256'] == original['input_sha256']
        assert a['metadata']['command'] == original['metadata']['command']
        exact = {n: sha(directories[0]/(n+'.csv')) == sha(directories[1]/(n+'.csv'))
                 for n in ['b','V','fereg_status','fereg_dof','fereg_certificate','fereg_fit']}
        assert exact['fereg_status'] and exact['fereg_dof']
        peaks = []
        for d,r in zip(directories,[b,a]):
            assert r['metadata']['clock_empty_mean'] < 1e-5
            assert r['metadata']['clock_empty_max'] < 1e-4
            assert 0 < r['metadata']['highres_seconds'] < 1
            assert abs(r['metadata']['highres_seconds'] - r['seconds']) < .002
            assert all(sha(root/'controller'/n)==h for n,h in r['measurement_fingerprint']['controller'].items())
            peaks.append(int(next(x['value'] for x in csv.DictReader((d/'fereg_runtime.csv').open())
                                  if x['row']=='13' and x['column']=='2')))
        assert peaks[0] == peaks[1]
        pairs.append(dict(label=label,rep=rep,candidate_seconds=a['metadata']['highres_seconds'],
            baseline_seconds=b['metadata']['highres_seconds'],
            highres_ratio=a['metadata']['highres_seconds']/b['metadata']['highres_seconds'],
            ordinary_ratio=a['seconds']/b['seconds'],baseline_timer_seconds=b['seconds'],candidate_timer_seconds=a['seconds'],
            calibration={v:{k:r['metadata'][k] for k in ['clock_empty_mean','clock_empty_min','clock_empty_max']}
                         for v,r in [('baseline',b),('usefulpilot',a)]},exact=exact,
            peak_baseline=peaks[0],peak_candidate=peaks[1],input_sha256=a['input_sha256'],
            result_hashes=[sha(d/'result.json') for d in directories]))
    block = [p for p in pairs if p['label']==label]
    blocks.append(dict(label=label,pairs=len(block),highres_median_ratio=statistics.median(p['highres_ratio'] for p in block),
                       ordinary_median_ratio=statistics.median(p['ordinary_ratio'] for p in block)))
assert len(pairs)==10 and all(b['highres_median_ratio']<=1.05 for b in blocks)
for variant in ['baseline','usefulpilot']:
    native = json.loads((base/f'identity-{variant}.json').read_text())
    assert native['abi']=='pass' and sha(base/variant/'dist/fereg_macos.plugin')==native['plugin']
candidate = json.loads((base/'identity-usefulpilot.json').read_text())
assert all(sha(Path(p))==h for p,h in candidate['source'].items())
archive(root, output, labels)
write(output/'precision-audit.json',dict(status='pass',blocks=blocks,pairs=pairs,
    source_files_verified=len(candidate['source']),earlier_failures_preserved=True,
    native_control_threshold=1.05,criterion='both predeclared high-resolution block medians <=1.05',
    gate_disposition='resolved by same-command precision measurement; earlier rounded checks remain failed',
    total_successful_calls=20,bitwise_exact_pairs=sum(all(p['exact'].values()) for p in pairs)))
shutil.copy(root/'precision-plan.json',output)
shutil.copy(root/'clock/fixture.json',output/'clock-fixture.json')
for name in ['highres.py','highres_clock.c','check_highres_clock.do']:
    shutil.copy(Path('benchmarks/two_fe_dense')/name,output/'harness'/name)
for name in ['uninstrumented-stata-worker.do','uninstrumented-manifest.json','worker.patch']:
    shutil.copy(root/'clock'/name,output/name)
(output/'controller').mkdir()
for p in (root/'controller').iterdir():
    if p.is_file():shutil.copy(p,output/'controller'/p.name)
failures=[]
for attempt in ['small-clock','small-clock-v2']:
    p=base/attempt/'clock/build.txt'
    failures.append(dict(attempt=attempt,raw_log_sha256=sha(p),
        diagnostic=p.read_text().replace(str(Path.cwd()),'<repo>')))
write(output/'build-failures.json',failures)
print('PRECISION_AUDIT_PASS',blocks,'exact pairs',sum(all(p['exact'].values()) for p in pairs),flush=True)
