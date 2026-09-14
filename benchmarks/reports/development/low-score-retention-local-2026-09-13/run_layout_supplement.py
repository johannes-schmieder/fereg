import os
from pathlib import Path
import os,json,hashlib,subprocess,re
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((root/'built-identities.json').read_text());rows=[]
cases=[('unsupported-baseline','baseline','layout_oracle.do',498,'unsupported: prepare-ols-test supports only'),('generated-baseline','baseline','generated_supported_oracle.do',0,'END_FIXTURE|low_score_generated_supported|fits=1|rc=0'),('generated-candidate','candidate','generated_supported_oracle.do',0,'END_FIXTURE|low_score_generated_supported|fits=1|rc=0')]
for name,arm,fixture,expected,sentinel in cases:
 source=root/arm;assert sha(source/'dist/fereg_macos.plugin')==ids[arm+'_plugin'];out=root/'validation'/name;out.mkdir(parents=True,exist_ok=False)
 (out/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{root/fixture}"\nlocal result=_rc\ndisplay "LAYOUT_SUPPLEMENT_RC=`result\'"\nexit `result\'\n')
 env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env['FEREG_PHASE_TIMING']='1'
 p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(out/'run.do')],cwd=out,env=env,capture_output=True,timeout=240)
 (out/'process.stdout').write_bytes(p.stdout);(out/'process.stderr').write_bytes(p.stderr);txt=(out/'run.log').read_text(errors='replace')
 row={'name':name,'arm':arm,'fixture_sha256':sha(root/fixture),'plugin':ids[arm+'_plugin'],'process_rc':p.returncode,'terminal_rc':re.findall(r'^LAYOUT_SUPPLEMENT_RC=(\d+)\s*$',txt,re.M),'expected_rc':expected,'sentinel':sentinel in txt,'log_sha256':sha(out/'run.log'),'stderr_sha256':sha(out/'process.stderr')};rows.append(row);(root/'layout-supplement-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,row,flush=True)
 assert row['process_rc']==0 and row['terminal_rc']==[str(expected)] and row['sentinel'],name
print('LAYOUT_SUPPLEMENT_TERMINAL_PASS',flush=True)
