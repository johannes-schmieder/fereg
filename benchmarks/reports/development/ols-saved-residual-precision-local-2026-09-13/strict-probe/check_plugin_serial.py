import os
from pathlib import Path
import os,json,hashlib,subprocess
root=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(root/'plugin-serial-protocol.md').write_text('One complete serial plugin suite per accepted/candidate test binary, with the original assertions unchanged. This separates test-process concurrency from the intermittent allocation audit. Preserve the failed concurrent suite and all twenty fixed exact-test follow-ups. Do not repeat either full serial suite after failure.\n')
executables={'baseline':Path(os.path.join(os.environ['OMISSION_ROOT'], 'test-target/debug/deps/fereg_plugin-e9c3349472e39e19')),'candidate':root/'test-target/debug/deps/fereg_plugin-e9c3349472e39e19'}
env={k:v for k,v in os.environ.items() if not k.startswith('FEREG_')};env.update(RAYON_NUM_THREADS='4');rows=[]
for arm in ['baseline','candidate']:
 command=[str(executables[arm]),'--test-threads=1']
 with (root/(arm+'-plugin-serial.log')).open('wb') as log:p=subprocess.run(command,cwd=root/arm,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120)
 row={'arm':arm,'rc':p.returncode,'executable_sha256':sha(executables[arm]),'log_sha256':sha(root/(arm+'-plugin-serial.log')),'passed_95': 'test result: ok. 95 passed; 0 failed' in (root/(arm+'-plugin-serial.log')).read_text()};rows.append(row);(root/'plugin-serial-results.json').write_text(json.dumps(rows,indent=2)+'\n');print(row,flush=True)
assert all(x['rc']==0 and x['passed_95'] for x in rows)
print('PLUGIN_SERIAL_BASELINE_CANDIDATE_PASS',flush=True)
