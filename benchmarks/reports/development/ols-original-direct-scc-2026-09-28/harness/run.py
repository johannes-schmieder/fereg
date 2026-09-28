"""Untimed exact-input OLS adjudication; run only inside the bounded SCC job."""
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
sys.path.insert(0, str(root / 'systematic'))
from common import read_json, sha256, write_json
from generate import generate
from runner import measure, read_coefficients, sample_hash
from two_fe_direct_oracle import solve, relative_scores
from test_two_fe_direct_oracle import validate

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
output = root / 'results'
output.mkdir(exist_ok=False)
write_json(output / 'bindings.json', dict(original_runtime=bindings,
    source_manifest=sha256(root / 'source.json'), plugin=plan['plugin_sha256'],
    deployment=sha256(root / 'deployment.json'), job=os.environ['JOB_ID'],
    python=platform.python_version(), numpy=np.__version__, scipy=scipy.__version__, pandas=pd.__version__))
write_json(output / 'oracle-tests.json', validate())
print('DIRECT_ORACLE_SMALL_SVD_TESTS_PASS', flush=True)

original = a.reference / plan['original_case']
assert sha256(original / 'input.json') == plan['original_input_receipt_sha256']
assert sha256(original / 'fereg/result.json') == plan['original_native_result_sha256']
info = read_json(original / 'input.json')
cell = dict(model='ols', design=info['design'], n=info['n'], k=info['k'], threads=4,
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
answer = solve(y, X, frame.g1, frame.g2)
certificate = {k: answer[k] for k in ('rank', 'rss', 'certificate')}
certificate.update(coefficient=answer['coefficient'].tolist(), covariance=answer['covariance'].tolist())
write_json(output / 'oracle.json', certificate)
print('DIRECT_ORACLE_ORIGINAL_INPUT_PASS', certificate['rank'], certificate['rss'], flush=True)

expected_sample = hashlib.sha256(frame.obs_id.to_numpy().astype('<i8').tobytes()).hexdigest()
tolerance = plan['science']
def assess(b, V, rss, rank, sample, converged):
    close = lambda x, z, what: bool(np.allclose(x, z, rtol=tolerance[what + '_rtol'], atol=tolerance[what + '_atol']))
    checks = dict(coefficients=close(b, answer['coefficient'], 'coefficient'),
                  covariance=close(V, answer['covariance'], 'vce'),
                  standard_errors=close(np.sqrt(np.diag(V)), np.sqrt(np.diag(answer['covariance'])), 'vce'),
                  objective=close(rss, answer['rss'], 'objective'),
                  exact_rank=rank == answer['rank'], exact_sample=sample == expected_sample,
                  converged=converged)
    return dict(checks=checks, pass_all=all(checks.values()),
                coefficient_max_abs_error=float(np.max(abs(b - answer['coefficient']))),
                covariance_max_abs_error=float(np.max(abs(V - answer['covariance']))),
                rss_error=float(rss - answer['rss']), rank=rank)

results = dict(original={}, current={}, new_reference={})
def assess_receipt(folder):
    receipt = read_json(folder / 'result.json')
    assert receipt['status'] == 'pass' and receipt['input_sha256'] == info['sha256']
    coefficients = read_coefficients(folder)
    with (folder / 'covariance.csv').open() as stream:
        covariance = {(r['row'], r['column']): float(r['value']) for r in csv.DictReader(stream)}
    b = np.array([coefficients[n][0] for n in names])
    V = np.array([[covariance[(n, m)] for m in names] for n in names])
    meta = receipt['metadata']
    rank = meta.get('df_k', meta['N'] - meta.get('df_r', float('nan')))
    result = assess(b, V, meta['rss'], rank, receipt['sample_sha256'], meta.get('converged') == 1)
    result['files'] = {f: sha256(folder / f) for f in ('result.json', 'coefficients.csv', 'covariance.csv')}
    return result

for arm in ('fereg', 'fixest', 'areg', 'pyfixest'):
    results['original'][arm] = assess_receipt(original / arm)
write_json(output / 'comparison.json', results)
work = root / 'worker-root'
work.mkdir(exist_ok=False)
for name, target in [('candidate', a.source), ('references', a.reference / 'references'), ('env', a.reference / 'env')]:
    (work / name).symlink_to(target, target_is_directory=True)
generate(dict(cell, n=manifest['measurement']['warmup_rows']), info['seed'], warm, manifest)
measure(work, cell, 'fereg', 'warm', data, warm, output / 'native', manifest,
        sha256(output / 'bindings.json'), limits=plan['native_limits'])
results['current']['fereg'] = assess_receipt(output / 'native')
write_json(output / 'comparison.json', results)

binary = a.scratch / 'input.bin'
with binary.open('wb') as stream:
    for column in frame:
        stream.write(np.asarray(frame[column], dtype='<f8').tobytes())
(a.scratch / 'columns.txt').write_text('\n'.join(frame.columns) + '\n')
for arm in plan['reference_arms']:
    folder = output / arm
    folder.mkdir()
    with (folder / 'reference.log').open('w') as log:
        subprocess.run(['Rscript', str(root / 'reference.R'), str(a.reference / 'env/R'),
                        str(a.scratch), str(folder), arm], check=True, timeout=plan['reference_timeout_seconds'],
                       stdout=log, stderr=subprocess.STDOUT)
    assert 'OLS_DIRECT_REFERENCE_COMPLETE' in (folder / 'reference.log').read_text()
    assert sha256(folder / 'r-input.bin') == sha256(binary)
    (folder / 'r-input.bin').unlink()
    b = pd.read_csv(folder / 'b.csv')
    assert list(b.name) == names
    V = np.loadtxt(folder / 'V.csv', delimiter=',')
    meta = pd.read_csv(folder / 'meta.csv').iloc[0].to_dict()
    sample, n = sample_hash(folder / 'sample.csv')
    assert n == len(y)
    result = assess(b.coefficient.to_numpy(), V, meta['rss'], meta['df_k'], sample, bool(meta['converged']))
    residual = np.fromfile(folder / 'residual.bin', dtype='<f8')
    assert residual.shape == y.shape
    result['residual_scores'] = relative_scores(X, frame.g1.to_numpy() - 1, frame.g2.to_numpy() - 1,
                                               np.ones(len(y)), residual)
    result['residual_max_abs_error'] = float(np.max(abs(residual - answer['residual'])))
    result['iterations'] = (folder / 'iterations.txt').read_text().strip()
    result['sample_sha256'] = sample
    result['metadata'] = meta
    results['new_reference'][arm] = result
    write_json(output / 'comparison.json', results)
    print('REFERENCE_ASSESSED', arm, result, flush=True)
write_json(output / 'complete.json', dict(complete=True, original_arms=4, current_native=1, new_reference=3,
           oracle_pass=True, diagnostic_only=True, no_timing_rank=True,
           current_native_science=results['current']['fereg']['pass_all']))
print('FEREG_OLS_DIRECT_ADJUDICATION_COMPLETE', flush=True)
