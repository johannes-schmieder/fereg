"""Original PPML coefficient adjudication; no timing eligibility is granted."""
import argparse
import csv
import hashlib
import os
from pathlib import Path
import platform
import subprocess
import sys

import numpy as np
import pandas as pd
import scipy

p = argparse.ArgumentParser()
for name in ('run-root', 'reference', 'source', 'scratch'):
    p.add_argument('--' + name, type=Path, required=True)
a = p.parse_args()
root = a.run_root.resolve()
sys.path[:0] = [str(root / 'systematic'), str(root / 'scripts')]
from common import read_json, sha256, write_json
from generate import generate
from runner import measure, sample_hash
from two_fe_ppml_oracle import fit
from test_two_fe_ppml_oracle import validate
from ppml_coefficient_accuracy import two_fe_coefficient_accuracy, Decimal, LOW, HIGH, point
from ppml_auto_science import compare_optimum_intervals

assert os.environ['NSLOTS'] == '4'
plan = read_json(root / 'registration.json')
for name, digest in read_json(root / 'deployment.json').items():
    assert sha256(root / name) == digest, name
source = read_json(root / 'source.json')
for name, digest in source['files'].items():
    assert sha256(a.source / name) == digest, name
assert sha256(a.source / 'dist/fereg_linux.plugin') == plan['plugin_sha256']
frozen = read_json(a.reference / 'env/frozen.json')
assert frozen['fingerprint'] == plan['original_fingerprint']
bindings = {}
for name, digest in frozen['runtime'].items():
    if name.startswith('env/R/fixest/') or name in ('env/python-freeze.txt', 'env/R-packages.csv'):
        assert sha256(a.reference / name) == digest, name
        bindings[name] = digest
assert sha256(root / 'systematic/generate.py') == frozen['controller']['generate.py']
manifest = read_json(a.reference / 'input/manifest.json')
assert manifest['science'] == plan['science']
for profile in plan['reference_profiles'].values():
    assert 10000 * np.finfo(float).eps < profile['fixef_tol'] <= 1e-11
    assert 0 < profile['glm_tol'] <= 1e-10
for path, digest in plan['original_receipts'].items():
    assert sha256(a.reference / path) == digest, path
output = root / 'results'
output.mkdir(exist_ok=False)
write_json(output / 'bindings.json', dict(original_runtime=bindings,
    source_manifest=sha256(root / 'source.json'), plugin=plan['plugin_sha256'],
    deployment=sha256(root / 'deployment.json'), job=os.environ['JOB_ID'],
    python=platform.python_version(), numpy=np.__version__, scipy=scipy.__version__, pandas=pd.__version__))
write_json(output / 'oracle-tests.json', validate())
print('PPML_SMALL_EXPLICIT_ORACLES_PASS', flush=True)
original = a.reference / plan['original_cases'][0]
info = read_json(original / 'input.json')
cell = dict(model='ppml', design=info['design'], n=info['n'], k=info['k'], threads=4,
            section=info['section'], effects=['g1', 'g2'])
a.scratch.mkdir(exist_ok=False)
data, warm = a.scratch / 'input.dta', a.scratch / 'warm.dta'
generated = generate(cell, info['seed'], data, manifest)
assert generated['sha256'] == info['sha256'] == plan['input_sha256']
assert generated['column_sha256'] == info['column_sha256']
write_json(output / 'input.json', info)
frame = pd.read_stata(data, convert_categoricals=False)
names = [f'x{j}' for j in range(1, cell['k'] + 1)]
X, y = frame[names].to_numpy(), frame.y.to_numpy()
assert (y > 0).all() and min(info['minimum_level_rows'].values()) >= 2
answer = fit(y, X, frame.g1, frame.g2)
np.savez(output / 'anchor.npz', beta=answer['beta'], mu=answer['mu'], covariance=answer['covariance'])
write_json(output / 'oracle.json', {k: v.tolist() if isinstance(v, np.ndarray) else v
                                   for k, v in answer.items() if k != 'mu'})
print('PPML_FULL_DIRECT_ANCHOR_PASS', answer['rank'], answer['deviance'], flush=True)
proof = two_fe_coefficient_accuracy(y, X, answer['beta'], answer['mu'], frame[['g1', 'g2']].to_numpy())
write_json(output / 'optimum-certificate.json', proof)
assert proof['coefficient_accuracy_certified']
radius = Decimal(proof['coefficient_radius_upper'])
intervals = [(str(LOW.subtract(point(b)[0], radius)), str(HIGH.add(point(b)[0], radius)))
             for b in answer['beta']]
write_json(output / 'optimum-intervals.json', intervals)
print('PPML_OPTIMUM_ENCLOSED', str(radius), flush=True)
expected_sample = hashlib.sha256(frame.obs_id.to_numpy().astype('<i8').tobytes()).hexdigest()
tolerance = plan['science']

def assess(folder, native=False, historical=True):
    with (folder / 'coefficients.csv').open() as stream:
        coefficients = {r['name']: r for r in csv.DictReader(stream)}
    tokens = [coefficients[n]['value'].strip() for n in names]
    b = np.array(list(map(float, tokens)))
    with (folder / 'covariance.csv').open() as stream:
        covariance = {(r['row'], r['column']): float(r['value']) for r in csv.DictReader(stream)}
    V = np.array([[covariance[(n, m)] for m in names] for n in names])
    if historical:
        receipt = read_json(folder / 'result.json')
        assert receipt['status'] == 'pass' and receipt['input_sha256'] == info['sha256']
        meta, sample = receipt['metadata'], receipt['sample_sha256']
    else:
        meta = read_json(folder / 'metadata.json')
        sample, count = sample_hash(folder / 'sample.csv')
        assert count == len(y)
    rank = meta['rank'] + meta['df_a'] if native else meta['df_k']
    close = lambda x, z, what: bool(np.allclose(x, z, rtol=tolerance[what + '_rtol'], atol=tolerance[what + '_atol']))
    # Using the anchor as candidate here classifies the estimator's printed
    # coordinates against certified optimum intervals, including decimal units.
    interval_check = compare_optimum_intervals(answer['beta'], tokens, intervals,
                                             rtol=str(tolerance['coefficient_rtol']),
                                             atol=str(tolerance['coefficient_atol']))
    estimator_bound = compare_optimum_intervals(b, tokens, intervals,
                                              rtol=str(tolerance['coefficient_rtol']),
                                              atol=str(tolerance['coefficient_atol']))
    checks = dict(coefficients_point=close(b, answer['beta'], 'coefficient'),
                  coefficients_certified=estimator_bound['candidate_inside_optimum_envelope'],
                  covariance_point=close(V, answer['covariance'], 'vce'),
                  standard_errors_point=close([float(coefficients[n]['se']) for n in names], np.sqrt(np.diag(answer['covariance'])), 'vce'),
                  objective=close(meta['deviance'], answer['deviance'], 'objective'),
                  exact_rank=rank == answer['rank'], exact_sample=sample == expected_sample,
                  converged=meta.get('converged') == 1)
    return dict(checks=checks, pass_all=all(checks.values()), metadata=meta,
                coefficient_max_abs_error=float(np.max(abs(b - answer['beta']))),
                covariance_max_abs_error=float(np.max(abs(V - answer['covariance']))),
                outside_optimum_envelope=[r['coordinate'] for r in interval_check['coordinates'] if r['reference_outside_optimum_envelope']],
                interval_check=interval_check, candidate_bound=estimator_bound,
                files={f: sha256(folder / f) for f in ('coefficients.csv', 'covariance.csv')})

results = dict(original={}, current={}, new_reference={})
for path in plan['original_cases']:
    folder = a.reference / path
    for arm in ('feppml', 'fixest_strict'):
        results['original'][path + '/' + arm] = assess(folder / arm, native=arm == 'feppml')
write_json(output / 'comparison.json', results)
work = root / 'worker-root'
work.mkdir(exist_ok=False)
for name, target in [('candidate', a.source), ('references', a.reference / 'references'), ('env', a.reference / 'env')]:
    (work / name).symlink_to(target, target_is_directory=True)
generate(dict(cell, n=manifest['measurement']['warmup_rows']), info['seed'], warm, manifest)
native = measure(work, cell, 'feppml', 'warm', data, warm, output / 'native', manifest,
                 sha256(output / 'bindings.json'), limits=plan['native_limits'])
assert native['metadata'].get('feppml_threads') == 4
assert native['metadata'].get('feppml_requested_threads') == 4
results['current']['feppml'] = assess(output / 'native', native=True)
write_json(output / 'comparison.json', results)
binary = a.scratch / 'input.bin'
with binary.open('wb') as stream:
    for column in frame:
        stream.write(np.asarray(frame[column], dtype='<f8').tobytes())
(a.scratch / 'columns.txt').write_text('\n'.join(frame.columns) + '\n')
for name, settings in plan['reference_profiles'].items():
    folder = output / name
    folder.mkdir()
    write_json(folder / 'settings.json', settings)
    with (folder / 'reference.log').open('w') as log:
        subprocess.run(['Rscript', str(root / 'reference.R'), str(a.reference / 'env/R'),
                        str(a.scratch), str(folder), str(folder / 'settings.json')], check=True,
                       timeout=plan['reference_timeout_seconds'], stdout=log, stderr=subprocess.STDOUT)
    assert 'PPML_ORIGINAL_REFERENCE_COMPLETE' in (folder / 'reference.log').read_text()
    assert sha256(folder / 'r-input.bin') == sha256(binary)
    (folder / 'r-input.bin').unlink()
    result = assess(folder, historical=False)
    mu = np.fromfile(folder / 'means.bin', dtype='<f8')
    assert mu.shape == y.shape and (mu > 0).all() and np.isfinite(mu).all()
    result['maximum_mean_point_error'] = float(np.max(abs(mu - answer['mu'])))
    results['new_reference'][name] = result
    write_json(output / 'comparison.json', results)
    print('REFERENCE_ASSESSED', name, result['checks'], result['outside_optimum_envelope'], flush=True)
write_json(output / 'complete.json', dict(complete=True, original_scopes=2, current_native=1, new_reference=4,
           oracle_pass=True, diagnostic_only=True, no_timing_rank=True,
           current_native_science=results['current']['feppml']['pass_all']))
print('FEREG_PPML_ORIGINAL_POSITIVE_COMPLETE', flush=True)
