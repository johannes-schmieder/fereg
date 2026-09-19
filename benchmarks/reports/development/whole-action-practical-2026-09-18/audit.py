#!/usr/bin/env python3
"""Replay development memory and non-regression gates without fitting models."""
from pathlib import Path
import csv
import hashlib
import importlib.util
import json
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def read(name):
    return json.loads((ROOT / name).read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


for relative, expected in read('evidence-sha256.json').items():
    assert sha(ROOT / relative) == expected, relative

spec = importlib.util.spec_from_file_location('memory_audit', REPO / 'scripts/audit_pragmatic_memory.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
calibration = read('calibration.json')
assert len(calibration) == 16
with zipfile.ZipFile(ROOT / 'diagnostics.zip') as archive, tempfile.TemporaryDirectory() as tmp:
    for row in calibration:
        work = Path(tmp) / row['case']
        work.mkdir()
        arm = row['receipt']['arm']
        prefix = f"runs/{row['case']}/calibration/"
        for name in (arm + '_memory.csv', 'stderr.txt', 'result.json'):
            (work / name).write_bytes(archive.read(prefix + name))
        result = audit.audit_case(work)
        assert result == row['memory_audit'], row['case']
        assert all(result[k] for k in ('native_accuracy_pass', 'process_accuracy_pass', 'practical_estimate_present'))
        assert not result['whole_action_accepted']  # Individual receipts cannot promote the milestone.

pairs = read('pairs.json')
assert len(pairs) == 9
assert all(p['science'] == 'pass' for p in pairs)
assert all(r['status'] == 'pass' and r['sentinel'] for p in pairs for r in p['arms'].values())
assert all(r['all_exact'] and r['sample_sha256_exact'] for r in read('science-exact.json'))
assert not any(r['flag'] for r in read('summary.json')['cases'])
assert not any(r['timing_flags'] for r in read('phase-summary.json'))

for group in ('mac-posting/holdout', 'linux-posting'):
    cases = read(group + '/cases.json')
    assert len(cases) == 4
    for name, n, k, levels in cases:
        phases = {}
        with (ROOT / group / name / 'probe.tsv').open() as stream:
            for phase, key, value in csv.reader(stream, delimiter='\t'):
                phases.setdefault(phase, {})[key] = int(value.strip())
        def allocated(phase):
            return sum(v for key, v in phases[phase].items() if key.endswith('_a'))
        before = allocated('held')
        observed = max(0, max(allocated(p) for p in ('posted', 'unheld', 'returned')) - before)
        segment = 32 << 20
        factor = ((max(20*n, before//10) + segment-1)//segment)*segment if levels else 0
        dim = k + levels + 1
        predicted = factor + dim*(dim+1)*24 + (64 << 10)
        assert abs(predicted-observed) <= max(64 << 20, observed*.1), (group, name)
        assert (ROOT / group / name / 'result.txt').read_text().strip() == 'STATA_MEMORY_PROBE_PASS'

german = read('german-replay.json')
prior = REPO / 'benchmarks/reports/development/cmg-expected-heap-2026-09-18/german/validation.json'
assert sha(prior) == german['prior_validation_sha256']
old = json.loads(prior.read_text())
assert old['science_pass'] and old['retained_rows'] == 4_361_733
assert german['native_predicted_bytes'] == old['memory_audit']['native_predicted_bytes']
whole = old['memory_audit']['stata_allocated_before'] + max(german['native_predicted_bytes'], german['publication_predicted_bytes'])
assert whole == german['process_predicted_bytes']
assert abs(whole-old['process_peak_rss_bytes']) <= max(64 << 20, old['process_peak_rss_bytes']*.1)
assert not german['reported_status2_on_German_candidate']

def scheduler(name):
    return dict(line.split(None, 1) for line in (ROOT / name).read_text().splitlines() if len(line.split(None, 1)) == 2)

original = scheduler('qacct-7641143.txt')
final = scheduler('qacct-7641149.txt')
assert original['failed'].strip() == '0' and original['exit_status'].strip() == '2'
assert final['failed'].strip() == '0' and final['exit_status'].strip() == '0'
assert 'FEREG_WHOLE_MEMORY_PASS' in (ROOT / 'DONE').read_text()
assert not read('wrapper-recheck.json')['repeated_estimator_measurements']
assert read('identities.json')['practical_milestone_accepted']
assert not read('identities.json')['strict_proof_complete']
assert not read('identities.json')['qualification']
print('WHOLE_ACTION_PRACTICAL_PASS: 16 estimator cases, 8 platform posting holdouts, German replay, 9 exact scientific pairs; strict proof/qualification remain separate')
