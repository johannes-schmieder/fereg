from pathlib import Path
import json,zipfile
from collect_preparation import collect as collect_ols
from collect_ppml_preparation import collect as collect_ppml
from common import read_json,write_json,sha256
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2])
note='Bounded local development: three rotated primary seeds, single-pair protected controls, and separate public phase profiles/pilot. No private allocationaudit profile. Source-bound numeric receipts; native/model from the result wire and RSS from process sampling. No release or cross-platform claim.'
collect_ols(root,out/'ols',pair_prefix='prep-interaction',primary_cases=['ols_interacted_cluster','ols_interacted_fe_cluster'],note=note)
collect_ppml(root,out/'ppml',prefix='ppmlprep-interaction',primary_cases={'ppml_interacted_fe_cluster'})
p=out/'ppml/comparisons.json';data=read_json(p);data['note']=note;write_json(p,data)
with zipfile.ZipFile(out/'ols/numeric-evidence.zip','a',zipfile.ZIP_DEFLATED) as archive:
 for p in sorted((root/'bounded-interaction-data').glob('*.json')):archive.write(p,'bounded-inputs/'+p.name)
p=out/'ols/archive.json';data=read_json(p);data['sha256']=sha256(out/'ols/numeric-evidence.zip');write_json(p,data)
for model in ['ols','ppml']:
 d=read_json(out/model/'comparisons.json')
 for r in d['summary']:
  print('DETAIL',model,json.dumps(r))
 for r in d['pairs']:
  for arm in ['baseline','candidate']:
   v=r[arm]
   if v['native_peak_bytes'] > v['modeled_bytes']:print('MODEL GAP',model,r['pair'],arm,v['native_peak_bytes'],v['modeled_bytes'])
print('COMPLETED')
