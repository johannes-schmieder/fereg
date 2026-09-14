import os
from pathlib import Path
import json,os,subprocess,re,hashlib,sys
root=Path(__file__).resolve().parent;label=sys.argv[1];usable=int(sys.argv[2]);expected=sys.argv[3];cap=536870912+usable
source=root/'candidate';ids=json.loads((root/'built-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(source/'dist/fereg_macos.plugin')==ids['candidate_plugin']
request=next(x for x in json.loads((root/'control-requests.json').read_text()) if x['name']=='slope');data=Path(request['data']);assert sha(data)==request['data_sha256']
work=root/'slope-caps'/label;work.mkdir(parents=True,exist_ok=False)
fixture=root/'slope_cap.do'
(work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\ncapture noisily do "{fixture}" {expected} "{data}"\nlocal actual=_rc\ndisplay "VALIDATION_TERMINAL_RC=`actual\'"\nexit `actual\'\n')
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
(work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr);txt=(work/'run.log').read_text(errors='replace')
row=dict(label=label,cap=cap,usable=usable,process_rc=p.returncode,plugin=ids['candidate_plugin'],input_sha256=sha(data),fixture_sha256=sha(fixture),log_sha256=sha(work/'run.log'),actual_rc=re.findall(r'^SLOPE_CAP_ACTUAL_RC=(\d+)',txt,re.M),terminal_rc=re.findall(r'^VALIDATION_TERMINAL_RC=(\d+)',txt,re.M),diagnostics=[x for x in txt.splitlines() if x.startswith('fereg:') or x.startswith('SLOPE_CAP_')])
(work/'result.json').write_text(json.dumps(row,indent=2)+'\n');print(json.dumps(row,indent=2),flush=True)
assert p.returncode==0 and row['terminal_rc']==['0'] and re.search(r'^SLOPE_CAP_PASS rc=0\s*$',txt,re.M)
