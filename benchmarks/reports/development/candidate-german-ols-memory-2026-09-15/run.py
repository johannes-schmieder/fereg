from pathlib import Path
import hashlib, json, os, subprocess, time
import numpy as np
import pandas as pd

root = Path(__file__).resolve().parent
screen = Path('${SCREEN_ROOT}')
prior = Path('${PRESERVED_GERMAN_ROOT}')
source = screen / 'source'
data = prior / 'input/sample.dta'
old = prior / 'comparison-ols-isolated-1/ols-native-1'

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda: f.read(8 << 20), b''):
            h.update(b)
    return h.hexdigest()

identity = json.loads((screen / 'identity.json').read_text())
for rel, digest in identity['files'].items():
    assert sha(source / rel) == digest, rel
assert sha(source / 'dist/fereg_linux.plugin') == '4782babab9875814045d12f4f93d4107065ffca0b855c218ee2114d5061d4642'
assert sha(data) == '6498d6051aac8cce1ff545c1a0228f655fcf7d6e88845b0c6f1ce564e6e7f59e'
out = root / 'native'
out.mkdir()
for name in ['empty-plus', 'empty-personal', 'compiled', 'scratch']:
    (out / name).mkdir()
body = f'''capture noisily do "{root}/fit.do" "{screen}" "{data}" "{out}" ols native 4 "{prior}/references"
local rc=_rc
file open result using "{out}/result.txt", write replace
file write result "STATA_RC=`rc'" _n
file close result
exit `rc'
'''
(out / 'run.do').write_text(body)
env = {k: v for k, v in os.environ.items() if not k.startswith('FEREG_')}
env.update(TMPDIR=str(out / 'scratch'), OMP_NUM_THREADS='4', OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1', NUMEXPR_NUM_THREADS='1', RAYON_NUM_THREADS='4', FEREG_MEMORY_LIMIT_BYTES='42949672960')
start = time.monotonic()
with (out / 'process.stdout').open('w') as stdout, (out / 'process.stderr').open('w') as stderr:
    fit = subprocess.run(['/usr/bin/time', '-v', 'stata-mp', '-b', 'do', 'run.do'], cwd=out, env=env, stdout=stdout, stderr=stderr, timeout=900)
receipt = dict(source_label=identity['label'], source_identity_sha256=sha(screen / 'identity.json'), plugin_sha256=sha(source / 'dist/fereg_linux.plugin'), input_sha256=sha(data), reference_lock_sha256=identity['reference_lock_sha256'], process_rc=fit.returncode, process_seconds=time.monotonic() - start, job_id=os.environ['JOB_ID'], host=os.uname().nodename, slots=int(os.environ['NSLOTS']), threads=4, native_limit_bytes=42949672960, accepted=False, no_reference_estimation=True, no_qualification=True)
(root / 'validation.json').write_text(json.dumps(receipt, indent=2) + '\n')
assert fit.returncode == 0
assert (out / 'result.txt').read_text().strip() == 'STATA_RC=0'
assert (out / 'fit-result.txt').read_text().startswith('FIT_RC=0\n')
assert 'GERMAN_FIVE_PERCENT_FIT_COMPLETE|family=ols|arm=native' in (out / 'run.log').read_text()
tables = ['b', 'V', 'fereg_status', 'fereg_fit', 'fereg_dof', 'fereg_fe', 'fereg_certificate']
for name in tables:
    assert (out / (name + '.tsv')).read_bytes() == (old / (name + '.tsv')).read_bytes(), name
left = pd.read_stata(old / 'outputs.dta', convert_categoricals=False)
right = pd.read_stata(out / 'outputs.dta', convert_categoricals=False)
assert left.columns.equals(right.columns)
for column in left.columns:
    assert np.array_equal(left[column].to_numpy(), right[column].to_numpy(), equal_nan=True), column
assert int(right['sample'].sum()) == 4361733
receipt.update(accepted=True, exact_scientific_tables=tables, all_saved_columns_exact=True, sample_order_exact=True, retained_rows=4361733, output_hashes={p.name: sha(p) for p in out.glob('*.tsv')}, literal_harness_hashes={p.name: sha(p) for p in [root / 'run.py', root / 'fit.do', root / 'run.sh']}, raw_log_hashes={n: sha(out / n) for n in ['run.log', 'process.stderr', 'fit-result.txt', 'result.txt']})
(root / 'validation.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('GERMAN_OLS_MEMORY_REPRODUCTION_PASS', flush=True)
