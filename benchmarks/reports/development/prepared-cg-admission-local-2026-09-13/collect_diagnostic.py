import os
from pathlib import Path
import json,hashlib
root=Path(os.environ['EXPERIMENT_ROOT']);rows=json.loads((root/'diagnostic-results.json').read_text())
for row in rows:
 p=root/'runs'/row['name']/'stderr.txt'
 row['audit']=[line for line in p.read_text().splitlines() if line.startswith(('ADMISSION_AUDIT|','RECOVERY_AUDIT|','RECOVERY_PLAN_AUDIT|'))]
 assert row['audit'];row['stderr_sha256']=hashlib.sha256(p.read_bytes()).hexdigest()
 checks=[]
 for line in row['audit']:
  if line.startswith('ADMISSION_AUDIT|'):
   fields=dict(part.split('=',1) for part in line.split('|')[1:]);fields['joint_bytes']=int(fields['execution'])+int(fields['increment']);checks.append(fields)
 row['admission_checks']=checks
 print(row['name'], [(r['file'].split('/')[-1],r['joint_bytes']) for r in checks])
(root/'diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n')
