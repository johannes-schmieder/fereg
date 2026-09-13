import os
from pathlib import Path
import json,hashlib,os,re,subprocess
ROOT=Path(__file__).parent;fixture=Path(os.path.join(os.environ['OMISSION_ROOT'], 'omitted_outputs_oracle.do'));source=ROOT/'candidate';ids=json.loads((ROOT/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
work=ROOT/'validation/omitted';work.mkdir(parents=True,exist_ok=False)
(work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" 0\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
(work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr)
text=(work/'run.log').read_text(errors='replace');sentinel='OMITTED_OUTPUTS_ORACLE|fits=4|expected_rc=0|rc=0'
row={'process_rc':p.returncode,'sentinel':bool(re.search('^'+re.escape(sentinel)+r'\s*$',text,re.M)),'log_sha256':sha(work/'run.log'),'fixture_sha256':sha(fixture),'plugin':ids['candidate_plugin']}
(ROOT/'omission-results.json').write_text(json.dumps(row,indent=2)+'\n');print(row,flush=True);assert p.returncode==0 and row['sentinel']
