from pathlib import Path
from collect_preparation import collect
import json,zipfile,hashlib
from run import BASE
root=BASE
for label,prefix in [('initial-pilot','inference-batch-pilot'),('revised-pilot','inference-batch-v2-pilot'),('primary','inference-batch-v2-primary'),('controls','inference-batch-v2-controls'),('many-small','inference-batch-v2-many-small'),('many-large','inference-batch-v2-many-large'),('profile','inference-batch-v2-profile'),('memory-rechecks','inference-batch-v2-memory-recheck')]:
 if not list((root/'runs').glob(prefix+'-*-candidate/inference-science.json')):continue
 out=root/'collected'/label
 collect(root,out,pair_prefix=prefix,science_marker='inference-science.json',exact_science=True,note='Isolated subset-score bank experiment. Separate initial/revised pilots, original primary/control cases, fresh many-way follow-ups and tracing; do not merge candidate artifacts or evidence tiers.',primary_cases=['low_fw_threeway','low_pw_multiway','low_manyway'] if label!='controls' else [])
 with zipfile.ZipFile(out/'numeric-evidence.zip','a',zipfile.ZIP_DEFLATED) as z:
  for p in sorted((root/'high-cardinality-data').glob('*.json')):z.write(p,'modified-inputs/'+p.name)
 p=out/'archive.json';v=json.loads(p.read_text());v['sha256']=hashlib.sha256((out/'numeric-evidence.zip').read_bytes()).hexdigest();p.write_text(json.dumps(v,indent=2)+'\n')
