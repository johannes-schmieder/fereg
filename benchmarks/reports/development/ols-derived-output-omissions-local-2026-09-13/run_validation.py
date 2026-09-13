import os
from pathlib import Path
import hashlib,json,os,re,subprocess,shutil
ROOT=Path(os.environ['OMISSION_ROOT'])
REPO=Path(os.environ['REPOSITORY_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=json.loads(Path(os.path.join(os.environ['SAVED_SCREEN_ROOT'], 'built-identities.json')).read_text())
ids={'baseline_commit':old['baseline_commit'],'baseline_plugin':old['baseline_plugin'],
     'candidate_plugin':sha(ROOT/'candidate/dist/fereg_macos.plugin'),
     'baseline_inputs':old['baseline_inputs'],
     'candidate_inputs':{p:sha(ROOT/'candidate'/p) for p in old['baseline_inputs']}}
ids['changes']=[p for p,h in ids['candidate_inputs'].items() if h!=ids['baseline_inputs'][p]]
assert ids['changes']==['crates/fereg-plugin/src/estimate.rs']
log=(ROOT/'build.log').read_text();assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2
assert 'actual-plugin ABI/self-test harness: PASS' in log
assert '95 passed; 0 failed' in (ROOT/'plugin-tests.log').read_text()
(ROOT/'identities.json').write_text(json.dumps(ids,indent=2)+'\n')
results=[]
cases=[('omitted-baseline','baseline',ROOT/'omitted_outputs_oracle.do','498','OMITTED_OUTPUTS_ORACLE|fits=4|expected_rc=498|rc=0'),
       ('omitted-candidate','candidate',ROOT/'omitted_outputs_oracle.do','0','OMITTED_OUTPUTS_ORACLE|fits=4|expected_rc=0|rc=0'),
       ('saved-candidate','candidate',Path(os.path.join(os.environ['SAVED_REPAIRED_ROOT'], 'oracle.do')),'0','SAVED_SCHUR_ORACLE|fits=48|rollback=1|rc=0')]
for name,arm,fixture,arg,sentinel in cases:
    work=ROOT/'validation'/name;work.mkdir(parents=True,exist_ok=False);source=ROOT/arm
    (work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" {arg}\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
    env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
    p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
    (work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr)
    log=(work/'run.log').read_text(errors='replace')
    row=dict(name=name,arm=arm,process_rc=p.returncode,sentinel=bool(re.search('^'+re.escape(sentinel)+r'\s*$',log,re.M)),log_sha256=sha(work/'run.log'),fixture_sha256=sha(fixture),plugin=ids[arm+'_plugin'])
    results.append(row);(ROOT/'validation-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(row),flush=True)
    assert row['sentinel'] and p.returncode==0,name
