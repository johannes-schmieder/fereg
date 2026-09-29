import concurrent.futures,datetime,hashlib,json,subprocess,sys
from pathlib import Path
base=Path('/projectnb/welfgr/fereg/dev');dest=base/'20260929-harness-repair-01';sys.path.insert(0,str(base/'20260928-solver-public-01'))
from accounting import parse
jobs=[('7766146','20260928-comparator-state-01',4),('7767016','20260928-nonretaining-cg-01',8),('7767269','20260928-ols-timeout-memory-01',4),('7767319','20260928-schwarz-imbalance-01',8),('7767737','20260928-solver-public-01',16)]
def capture(job):
 jid,name,slots=job;root=base/name
 raw=subprocess.run(['qacct','-j',jid],stdout=subprocess.PIPE,stderr=subprocess.PIPE,universal_newlines=True,check=True,timeout=120)
 account=parse(raw.stdout,jid,[1],slots)
 logs=list((root/'logs').glob('*.o'+jid));assert len(logs)==1
 return jid,dict(root=str(root),accounting=account,deployment_sha256=hashlib.sha256((root/'deployment.json').read_bytes()).hexdigest(),terminal_log_sha256=hashlib.sha256(logs[0].read_bytes()).hexdigest())
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:records=dict(pool.map(capture,jobs))
records['7766146']['cause']='Harness expected gtools_sample.log; actual gtools.log has completion marker and four passing checks'
records['7767016']['cause']='Missing compile-time tests/fixtures/ppml/reconstruction.csv; baseline test executable not built'
records['7767319']['cause']='Same missing compile-time fixture; candidate test executable not built'
records['7767737']['cause']='qacct subprocess exceeded 30-second timeout before candidate gates'
records['7767269']['cause']='Diagnostic complete: 448 GiB admission passes using LSMR; 192 GiB admission reaches 180-second process cap'
files={}
for rel in ['20260928-comparator-state-01/results/comparison.json','20260928-comparator-state-01/results/gtools/checks.tsv','20260928-comparator-state-01/results/gtools/receipt.json','20260928-ols-timeout-memory-01/output/complete.json','20260928-ols-timeout-memory-01/output/budget-448gib/receipt.json','20260928-ols-timeout-memory-01/output/budget-192gib/receipt.json']:
 p=base/rel;files[rel]=dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest(),text=p.read_text())
value=dict(captured_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),jobs=records,evidence=files)
(dest/'original-outcomes.json').write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
print('Captured five terminal diagnostic outcomes and six selected evidence files')
