from pathlib import Path
import subprocess, hashlib, json, os

root = Path(__file__).parent
records = []
for arm in ['baseline', 'candidate']:
    source = (root / arm).resolve()
    for name in ['feppml_one_fe_dummy_oracle']:
        selected = root / 'candidate/tests/stata/parity_ppmlhdfe' / (name + '.do')
        out = root / 'oracles-reviewed' / arm / name
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
display as result "ONE_FE_ORACLE_DONE|{name}|rc=`fixture_rc'"
exit `fixture_rc'
''')
        env = {k: v for k, v in os.environ.items() if not k.startswith('FEREG_')}
        with (out / 'process.log').open('w') as log:
            result = subprocess.run(['/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp',
                '-b', 'do', str(worker)], cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=300)
        log = (out / 'run.log').read_text(errors='replace')
        rc = int((out / 'rc.txt').read_text())
        sentinels = [f'END_FIXTURE|{name}|rc=0', f'ONE_FE_ORACLE_DONE|{name}|rc=0']
        assert result.returncode == rc == 0 and all(s in log for s in sentinels), (arm, name, rc)
        records.append(dict(arm=arm, fixture=name, rc=rc, process_rc=result.returncode,
            sentinels=sentinels, fixture_sha256=hashlib.sha256(selected.read_bytes()).hexdigest(),
            plugin_sha256=hashlib.sha256((source / 'dist/fereg_macos.plugin').read_bytes()).hexdigest(),
            log_sha256=hashlib.sha256((out / 'run.log').read_bytes()).hexdigest()))
        (root / 'oracle-results.json').write_text(json.dumps(records, indent=2) + '\n')
        print(arm, name, 'PASS', flush=True)
