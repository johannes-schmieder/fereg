from pathlib import Path
import subprocess,json,os
root=Path(__file__).resolve().parent
for name,n,k,levels in json.loads((root/'cases.json').read_text()):
 p=root/name
 r=subprocess.run([os.environ['STATA_MP_PATH'],'-b','do','probe.do'],cwd=p,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=90)
 assert r.returncode==0 and (p/'result.txt').read_text().strip()=='STATA_MEMORY_PROBE_PASS',name
 print(name,'STATA_MEMORY_PROBE_PASS',flush=True)
