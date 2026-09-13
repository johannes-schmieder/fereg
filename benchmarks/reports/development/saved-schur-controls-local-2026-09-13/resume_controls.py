import os
from pathlib import Path
import json
from compare import compare_pair
ROOT=Path(os.environ['SAVED_SCREEN_ROOT'])
adjudication=json.loads((ROOT/'weak-direct-results.json').read_text())
assert adjudication['baseline']['saved_e']['failing_rows']==1
assert all(adjudication['candidate'][col]['failing_rows']==0 for col in ['saved_e','saved_g','saved_h'])
assert adjudication['lsmr_residual_agreement_maxabs']<1e-10
assert adjudication['lsqr_residual_agreement_maxabs']<1e-10
requests=json.loads((ROOT/'control-requests.json').read_text())
assert [r['name'] for r in requests[:2]]==['weighted_cut','weak_d3']
assert len(json.loads((ROOT/'comparisons.json').read_text()))==10
for i,request in list(enumerate(requests))[2:]:
    compare_pair(request,i,'control')
