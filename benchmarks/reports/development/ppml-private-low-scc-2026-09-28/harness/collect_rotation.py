from pathlib import Path
import json,hashlib,subprocess,sys
num,job=sys.argv[1:];root=Path('/projectnb/welfgr/fereg/dev/20260928-private-low-rotation'+num)
raw=subprocess.check_output(['qacct','-j',job],universal_newlines=True)
fields={l.split(None,1)[0]:l.split(None,1)[1].strip() for l in raw.splitlines() if len(l.split())>=2}
assert fields['failed']==fields['exit_status']=='0'
log=(root/'job.log').read_text();assert 'FEREG_PRIVATE_LOW_TIMING_SUCCESS job='+job in log
sha=lambda b:hashlib.sha256(b).hexdigest()
bundle=dict(job=job,accounting={k:fields[k] for k in ['jobnumber','hostname','slots','failed','exit_status','ru_wallclock','cpu','maxvmem','start_time','end_time']},job_log_sha256=sha(log.encode()),sentinels=['FEREG_ROTATIONS_COMPLETE','FEREG_PRIVATE_LOW_TIMING_SUCCESS job='+job],blocks={})
for sub,n in [('',num)]+([('scaling','05')] if num=='04' else []):
 r=root/sub;results=r/('rotation'+n+'-results');summary=r/('summary'+n);d=json.loads((summary/'summary.json').read_text());assert d['complete'];files={}
 for p in results.rglob('*'):
  if p.is_file() and p.suffix in ['.json','.csv','.tsv']:
   b=p.read_bytes();files[str(p.relative_to(results))]=dict(text=b.decode(),sha256=sha(b))
 bundle['blocks'][n]=dict(files=files,summary={p.name:dict(text=p.read_text(),sha256=sha(p.read_bytes())) for p in summary.iterdir() if p.is_file()})
print(json.dumps(bundle))
