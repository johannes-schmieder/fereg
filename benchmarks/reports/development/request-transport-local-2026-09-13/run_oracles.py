from pathlib import Path
import subprocess, os, json, hashlib

root=Path(__file__).parent
records=[]
fixtures=[('development/within_weighted_ols',0),
          ('parity_ppmlhdfe/feppml_one_fe_dummy_oracle',0),
          ('development/local_request_transport_oracle',None)]
for arm in ['baseline','candidate']:
    source=root/arm
    for relative,expected in fixtures:
        name=Path(relative).name
        selected=root/'candidate/tests/stata'/(relative+'.do')
        expected=1003 if expected is None and arm=='baseline' else 0
        out=root/'oracles'/arm/name
        out.mkdir(parents=True,exist_ok=False)
        worker=out/'run.do'
        worker.write_text(f'''version 19
clear all
set more off
sysdir set PLUS "{root/'work/refs'}"
sysdir set PERSONAL "{out}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source/'stata'}"
adopath ++ "{source/'dist'}"
capture noisily do "{selected}"
local fixture_rc = _rc
file open result using "{out/'rc.txt'}", write text replace
file write result "`fixture_rc'" _n
file close result
display as result "TRANSPORT_ORACLE_DONE|{name}|rc=`fixture_rc'"
exit `fixture_rc'
''')
        env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
        with (out/'process.log').open('w') as log:
            run=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(worker)],cwd=out,env=env,
                stdout=log,stderr=subprocess.STDOUT,timeout=300)
        rc=int((out/'rc.txt').read_text())
        text=(out/'run.log').read_text(errors='replace')
        terminal=f'TRANSPORT_ORACLE_DONE|{name}|rc={expected}'
        sentinels=[terminal]+([f'END_FIXTURE|{name}|rc=0'] if expected==0 else [])
        records.append(dict(arm=arm,fixture=name,expected_rc=expected,rc=rc,process_rc=run.returncode,
            sentinels=sentinels,passed=run.returncode==0 and rc==expected and all(s in text for s in sentinels),
            fixture_sha256=hashlib.sha256(selected.read_bytes()).hexdigest(),
            plugin_sha256=hashlib.sha256((source/'dist/fereg_macos.plugin').read_bytes()).hexdigest(),
            log_sha256=hashlib.sha256((out/'run.log').read_bytes()).hexdigest()))
        (root/'oracle-results.json').write_text(json.dumps(records,indent=2)+'\n')
        print(arm,name,'rc',rc,'expected',expected,flush=True)
        assert records[-1]['passed'], (arm,name,rc)
