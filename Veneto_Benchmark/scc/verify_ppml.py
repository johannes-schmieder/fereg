"""Bounded full-panel verification against the preserved reference, not a campaign."""
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys

root, bench = map(Path, sys.argv[1:])
source = root / 'source'
data = bench / 'input/veneto-ppml.dta'
reference = bench / 'work/campaign/task-01/ppml-t1-r0-ppmlhdfe'

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(2**20), b''):
            h.update(block)
    return h.hexdigest()

def metadata(path):
    result = {}
    for line in path.read_text().splitlines():
        key, value = line.split('\t', 1)
        try:
            result[key] = float(value)
        except ValueError:
            result[key] = None if value.strip() == '.' else value
    return result

def table(path, key, columns):
    with path.open() as f:
        return {tuple(row[k] for k in key): [float(row[c]) for c in columns] for row in csv.DictReader(f)}

def comparison(actual, expected, tolerances):
    assert actual.keys() == expected.keys()
    checks = []
    for j, (rtol, atol) in enumerate(tolerances):
        gaps = [abs(actual[k][j] - expected[k][j]) for k in actual]
        passed = all(math.isfinite(actual[k][j]) and abs(actual[k][j] - expected[k][j]) <= atol + rtol * abs(expected[k][j]) for k in actual)
        checks.append({'passed': passed, 'max_absolute_gap': max(gaps), 'rtol': rtol, 'atol': atol})
    return checks

assert digest(data) == '92062a6bccaa7b55c3a213e288cf0d22e68668e21f454119ac168dbd938cb08d'
assert int(os.environ['NSLOTS']) == 14
ref_coef = table(reference / 'coefficients.csv', ['name'], ['value', 'se'])
ref_cov = table(reference / 'covariance.csv', ['row', 'column'], ['value'])
ref_meta = metadata(reference / 'metadata.tsv')
assert ref_meta['rc'] == 0 and ref_meta['converged'] == 1 and ref_meta['N'] == 3159720
reference_hashes = {name: digest(reference / name) for name in ['coefficients.csv', 'covariance.csv', 'metadata.tsv']}
affinity = sorted(os.sched_getaffinity(0))
assert len(affinity) >= 14
records = []
for threads in [4, 14]:
    output = root / 'results' / f't{threads}'
    output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.update(OMP_NUM_THREADS=str(threads), RAYON_NUM_THREADS=str(threads),
               OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1', NUMEXPR_NUM_THREADS='1',
               FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(output / 'solver-trace.txt'))
    command = ['taskset', '-c', ','.join(map(str, affinity[:threads])), 'stata-mp', '-b', 'do',
               str(root / 'verify_ppml.do'), str(source), str(bench / 'references'), str(data), str(threads), str(output)]
    with (output / 'stdout.txt').open('w') as stdout, (output / 'stderr.txt').open('w') as stderr:
        process = subprocess.Popen(command, cwd=output, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
        timeout = False
        try:
            process.wait(timeout=600)
        except subprocess.TimeoutExpired:
            timeout = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    meta = metadata(output / 'metadata.tsv') if (output / 'metadata.tsv').exists() else {}
    sentinel = any('VENETO_PPML_FIX_COMPLETE' in p.read_text() for p in output.glob('*.log'))
    record = {'job_id': os.environ['JOB_ID'], 'host': socket.gethostname(), 'threads': threads,
              'cpu_affinity': affinity[:threads], 'timeout': timeout, 'process_rc': process.returncode,
              'sentinel': sentinel, 'metadata': meta, 'input_sha256': digest(data),
              'plugin_sha256': digest(source / 'dist/fereg_linux.plugin'),
              'reference_sha256': reference_hashes, 'scientific_pass': False}
    if not timeout and process.returncode == 0 and sentinel and meta.get('rc') == 0:
        record['coefficient_and_se'] = comparison(table(output / 'coefficients.csv', ['name'], ['value', 'se']), ref_coef, [(1e-7, 1e-10), (1e-6, 1e-10)])
        record['covariance'] = comparison(table(output / 'covariance.csv', ['row', 'column'], ['value']), ref_cov, [(1e-6, 1e-10)])
        record['objectives'] = {}
        for key in ['ll', 'deviance']:
            value, ref = meta[key], ref_meta[key]
            record['objectives'][key] = {'value': value, 'reference': ref, 'gap': value-ref,
                                          'relative_gap': abs(value-ref)/max(abs(ref),1)}
        record['scientific_pass'] = (all(x['passed'] for x in record['coefficient_and_se'] + record['covariance'])
                                    and meta.get('converged') == 1 and meta.get('N') == 3159720
                                    and all(x['relative_gap'] <= 1e-7 for x in record['objectives'].values()))
    (output / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
    records.append(record)
    print('VENETO_PPML_FIX_RESULT', threads, record['scientific_pass'], flush=True)
    if not record['scientific_pass']:
        raise SystemExit('Focused Veneto check failed; preserved logs and receipt, no further fits launched.')
(root / 'COMPLETE.json').write_text(json.dumps(records, indent=2) + '\n')
print('VENETO_PPML_FIX_JOB_COMPLETE', flush=True)
