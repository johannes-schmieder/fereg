import hashlib, json, os, re, subprocess
from pathlib import Path

repo = Path(__file__).resolve().parents[2]
art = Path(__file__).resolve().parent
source = art / 'dtk1'
identity_path = art / 'identity-dtk1.json'
identity = json.loads(identity_path.read_text())
out = art / 'closeout1'
out.mkdir(exist_ok=False)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
for path, expected in identity['source'].items():
    assert sha(repo / path) == expected, path
    assert sha(source / path) == expected, path
assert sha(source / 'dist/fereg_macos.plugin') == identity['plugin']
(out / 'source.json').write_text(json.dumps(dict(
    schema='ols-round-closeout-source-v1',
    base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
    identity_sha256=sha(identity_path), source=identity['source'], plugin=identity['plugin'],
    matches_captured_dtk1=True), indent=2) + '\n')
tests = ['schur', 'fe::cmg::tests::', 'fe::lsmr_batch::tests::',
    'retained_serial_ols_window_model_preserves_other_admission_envelopes',
    'ols::prepared_fit::tests::narrow_weighted_final_overlap',
    'ols::prepared_fit::tests::two_fe_schur_final_overlap',
    'ols::prepared_fit::tests::residual_mean_bound',
    'ols::prepared_fit::tests::residual_bound_requires',
    'single_slope::tests::', 'single_slope_cache_is_ols_only_and_preserves_full_fit',
    'ppml_slope_only_topology_preserves_structural_constant']
env = dict(os.environ, DEVELOPER_DIR='/Library/Developer/CommandLineTools',
    CARGO_NET_OFFLINE='true', CARGO_TARGET_DIR=str(art / 'cargo-dtk1'),
    FEREG_GIT_COMMIT=identity['base_commit'][:8], FEREG_BUILD_DATE='2026-09-22')
records=[]
for i, test in enumerate(tests):
    log=out / f'core-{i+1}.txt'
    command=['cargo','test','--locked','--offline','--release','-p','fereg-core',
        '--target','aarch64-apple-darwin','--lib',test,'--','--nocapture']
    with log.open('x') as stream:
        result=subprocess.run(command,cwd=source,env=env,stdout=stream,stderr=subprocess.STDOUT)
    body=log.read_text()
    counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',body)
    passed=result.returncode==0 and len(counts)==1 and int(counts[0])>0
    records.append(dict(filter=test,passed=passed,count=int(counts[0]) if counts else None,
        returncode=result.returncode,log=log.name,log_sha256=sha(log),
        tests=re.findall(r'^test (\S+) \.\.\. ok$',body,re.M)))
    (out/'core-tests.json').write_text(json.dumps(dict(controller_sha256=sha(Path(__file__)),
        source_sha256=sha(out/'source.json'),status='pass' if len(records)==len(tests) and all(x['passed'] for x in records) else 'incomplete_or_failed',
        runs=records),indent=2)+'\n')
    print(test, records[-1]['count'],passed,flush=True)
    if not passed: raise SystemExit(1)
print('OLS_ROUND_CLOSEOUT_CORE_PASS',flush=True)
