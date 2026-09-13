import os
from pathlib import Path
import json
from compare import compare_pair
ROOT=Path(os.environ['SAVED_SCREEN_ROOT'])
for filename in ['weak-direct-results.json','disconnected_blocks-direct-results.json']:
    a=json.loads((ROOT/filename).read_text())
    assert a['baseline']['saved_e']['failing_rows']==1
    assert all(a['candidate'][col]['failing_rows']==0 for col in ['saved_e','saved_g','saved_h'])
    assert a['lsmr_residual_agreement_maxabs']<1e-10
    assert a['lsqr_residual_agreement_maxabs']<1e-10
requests=json.loads((ROOT/'control-requests.json').read_text())
assert [r['name'] for r in requests[:3]]==['weighted_cut','weak_d3','disconnected_blocks']
assert len(json.loads((ROOT/'comparisons.json').read_text()))==10
for i,request in list(enumerate(requests))[3:]:
    compare_pair(request,i,'control')
