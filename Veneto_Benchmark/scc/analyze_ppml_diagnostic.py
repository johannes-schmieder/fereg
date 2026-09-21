"""Audit the bounded diagnostic without altering the stopped benchmark results."""
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / 'work/ppml-diagnostic'
OUTPUT = ROOT / 'results/scc/ppml-diagnostic'
SOURCE = 'd44e36bfa5ad7e41a06752d1008f0b9683f8c8dd'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def frames(path):
    return [dict(part.split('=', 1) for part in line.split('|')[1:])
            for line in path.read_text().splitlines() if line.startswith('FEREG_SOLVER_TRACE|')]


records = json.loads((WORK / 'certificates/COMPLETE.json').read_text())
results = []
for receipt in records:
    assert receipt['job_id'] == '7535389'
    assert receipt['sentinel'] and not receipt['timeout'] and receipt['process_returncode'] == 0
    assert receipt['metadata']['rc'] == 430
    folder = WORK / 'certificates' / f"t{receipt['threads']}"
    rows = frames(folder / 'stderr.txt')
    reconstruction = [r for r in rows if r['phase'] == 'reconstruction_scale']
    certificate = [r for r in rows if r['phase'] == 'linear_output_certificate']
    assert len(reconstruction) == len(certificate) == 1
    scale = {k: float(v) for k, v in reconstruction[0].items() if k != 'phase'}
    cert = certificate[0]
    assert all('passed: true' in cert[k] for k in ['rank', 'score', 'bread', 'within'])
    assert cert['absorption'] == cert['residual_absorption'] == 'true'
    assert 'passed: false' in cert['reconstruction']
    assert scale['relative_error'] > scale['threshold']
    assert scale['termwise_error_max'] < 2.220446049250313e-16
    assert not any(r['phase'] == 'irls_convergence' for r in rows)
    log = '\n'.join(p.read_text() for p in folder.glob('*.log'))
    error = 'prepared PPML initial weighted-linear solve did not certify'
    assert error in log
    results.append({'threads': receipt['threads'], 'application_rc': 430,
                    'diagnostic_seconds': receipt['metadata']['seconds'],
                    'input_sha256': receipt['input_sha256'],
                    'plugin_sha256': receipt['plugin_sha256'],
                    'failed_certificate': 'predictor reconstruction',
                    'passed_certificates': ['rank', 'linear score', 'bread/inverse',
                                            'FE absorption', 'residual absorption', 'retained design inverse'],
                    'scale': scale, 'error_cutoff_ratio': scale['relative_error'] / scale['threshold'],
                    'certificate_fields': cert})

# Compare existing trace fields before the new logging point. Timings are excluded.
original = frames(WORK / 'original/t4/stderr.txt')
instrumented = frames(WORK / 'certificates/t4/stderr.txt')
def numerical(rows):
    return [{k: v for k, v in r.items() if not k.endswith('_seconds')}
            for r in rows if r['phase'] in ['lsmr_rhs', 'checkpoint_lsmr_attempt']]
left, right = numerical(original), numerical(instrumented)
assert len(left) == len(right)
trace_differences = []
for a, b in zip(left, right):
    assert a.keys() == b.keys()
    for key in a:
        if a[key] == b[key]:
            continue
        assert key not in ['phase', 'rhs', 'iterations', 'converged', 'canonical_passed']
        trace_differences.append({'phase': a['phase'], 'rhs': a.get('rhs'), 'field': key,
                                  'original': float(a[key]), 'instrumented': float(b[key]),
                                  'absolute_difference': abs(float(a[key]) - float(b[key]))})

# Verify every tracked source file against its frozen Git blob, not the dirty checkout.
repo = ROOT.parent
changed = []
checked = 0
for row in subprocess.check_output(['git', 'ls-tree', '-r', SOURCE], cwd=repo, text=True).splitlines():
    meta, name = row.split('\t', 1)
    mode, kind, expected = meta.split()
    assert kind == 'blob', (name, kind)
    p = WORK / 'instrumented-source' / name
    data = p.readlink().as_posix().encode() if p.is_symlink() else p.read_bytes()
    actual = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
    checked += 1
    if actual != expected:
        changed.append({'path': name, 'original_git_blob': expected, 'diagnostic_git_blob': actual,
                        'diagnostic_sha256': hashlib.sha256(data).hexdigest()})
assert [x['path'] for x in changed] == ['crates/fereg-core/src/ols/linear_session.rs']

summary = {'base_source': SOURCE, 'job_id': '7535389', 'host': 'scc-ei3', 'cpu_type': 'Gold-6226R',
           'comparison_is_benchmark': False, 'settings_changed': False,
           'original_four_thread_trace_exact_match': left == right,
           'original_four_thread_trace_same_routes_iterations_and_decisions': True,
           'original_four_thread_trace_numeric_differences': trace_differences,
           'tracked_source_files_checked': checked, 'changed_files': changed, 'fits': results,
           'evidence_sha256': {str(p.relative_to(WORK)): digest(p)
                               for folder in ['original', 'certificates']
                               for p in (WORK / folder).rglob('*') if p.is_file()}}
(OUTPUT / 'diagnosis.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps({'verified_fits': len(results), 'source_files_checked': checked,
                  'original_trace_exact_match': left == right,
                  'maximum_trace_absolute_difference': max(d['absolute_difference'] for d in trace_differences),
                  'cutoff_ratios': [r['error_cutoff_ratio'] for r in results]}, indent=2))
