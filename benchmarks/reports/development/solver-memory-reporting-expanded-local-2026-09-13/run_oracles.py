from pathlib import Path
import subprocess, hashlib, json, os

root = Path(os.environ['FEREG_LOCAL_BENCH_ROOT'])
fixture = root / 'within_weighted_cmg.do'
text = (root / 'candidate/tests/stata/development/within_weighted_ols.do').read_text()
assert 'solver(lsmr) preconditioner(schwarz)' in text
fixture.write_text(text.replace('solver(lsmr) preconditioner(schwarz)', 'solver(cmg)')
    .replace('END_FIXTURE|within_weighted_ols|rc=0', 'END_FIXTURE|within_weighted_cmg|rc=0'))
records = []
for arm in ['baseline', 'candidate']:
    source = (root / arm).resolve()
    for name in ['ols_lsmr_reuse', 'within_weighted_ols', 'within_weighted_cmg']:
        selected = fixture if name.endswith('_cmg') else source / ('tests/stata/development/' + name + '.do')
        out = root / 'oracles' / arm / name
        out.mkdir(parents=True, exist_ok=False)
        worker = out / 'run.do'
        worker.write_text(f'''version 19
clear all
set more off
sysdir set PLUS "{root / 'work/refs'}"
sysdir set PERSONAL "{out}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source / 'stata'}"
adopath ++ "{source / 'dist'}"
capture noisily do "{selected}"
local fixture_rc = _rc
file open result using "{out / 'rc.txt'}", write text replace
file write result "`fixture_rc'" _n
file close result
display as result "REPORTING_ORACLE_DONE|{name}|rc=`fixture_rc'"
exit `fixture_rc'
''')
        env = {k: v for k, v in os.environ.items() if not k.startswith('FEREG_')}
        with (out / 'process.log').open('w') as log:
            result = subprocess.run([os.environ['STATA_MP_PATH'],
                '-b', 'do', str(worker)], cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120)
        log = (out / 'run.log').read_text(errors='replace')
        rc = int((out / 'rc.txt').read_text())
        sentinels = [f'END_FIXTURE|{name}|rc=0', f'REPORTING_ORACLE_DONE|{name}|rc=0']
        assert result.returncode == rc == 0 and all(s in log for s in sentinels), (arm, name, rc)
        records.append(dict(arm=arm, fixture=name, rc=rc, process_rc=result.returncode,
            sentinels=sentinels, fixture_sha256=hashlib.sha256(selected.read_bytes()).hexdigest(),
            plugin_sha256=hashlib.sha256((source / 'dist/fereg_macos.plugin').read_bytes()).hexdigest(),
            log_sha256=hashlib.sha256((out / 'run.log').read_bytes()).hexdigest()))
        (root / 'oracle-results.json').write_text(json.dumps(records, indent=2) + '\n')
        print(arm, name, 'PASS', flush=True)
