import os
from pathlib import Path
import hashlib,json,os,re,subprocess
ROOT=Path(os.environ['SAVED_SCREEN_ROOT'])
request=next(x for x in json.loads((ROOT/'control-requests.json').read_text()) if x['name']=='slope')
results=[]
for arm in ['baseline','candidate']:
    source=ROOT/arm;work=ROOT/'slope-diagnostic'/arm;work.mkdir(parents=True,exist_ok=False)
    command='fereg y '+' '.join(f'x{i}' for i in range(1,17))+' [aw=w], absorb(saved_g=g1 g2#c.x1) threads(4) solver(auto) poolsize(10) vce(robust) residuals(saved_e)'
    (work/'run.do').write_text(f'clear all\nset more off\nadopath ++ "{source}/stata"\nadopath ++ "{source}/dist"\nuse "{request["warm"]}", clear\ncapture noisily {command}\nlocal actual=_rc\ndisplay "SLOPE_DIAGNOSTIC_RC=`actual\'"\nexit 0\n')
    env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')}
    p=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do',str(work/'run.do')],cwd=work,env=env,capture_output=True,timeout=180)
    (work/'process.stdout').write_bytes(p.stdout);(work/'process.stderr').write_bytes(p.stderr)
    log=(work/'run.log').read_text(errors='replace');matches=re.findall(r'^SLOPE_DIAGNOSTIC_RC=(\d+)\s*$',log,re.M)
    assert len(matches)==1 and p.returncode==0
    row=dict(arm=arm,rc=int(matches[0]),log_sha256=hashlib.sha256((work/'run.log').read_bytes()).hexdigest())
    results.append(row);print(json.dumps(row),flush=True)
(ROOT/'slope-diagnostic-results.json').write_text(json.dumps(results,indent=2)+'\n')
