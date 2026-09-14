import os
from pathlib import Path
import json,hashlib
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();j=json.loads((r/'diagnostic-built.json').read_text());j['baseline']=json.loads((r/'baseline-diagnostic-built.json').read_text())['baseline']
for arm in ['baseline','candidate']:
 assert sha(r/('diagnostic-'+arm)/'dist/fereg_macos.plugin')==j[arm]['plugin'];assert all(sha(r/('diagnostic-'+arm)/p)==h for p,h in j[arm]['inputs'].items())
(r/'diagnostic-built.json').write_text(json.dumps(j,indent=2)+'\n');print('BOTH_DIAGNOSTIC_IDENTITIES_VERIFIED')
