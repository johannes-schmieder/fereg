import os
from pathlib import Path
import copy,hashlib,json
from measure import measure
from run import MANIFEST
from final_compare import certify
ROOT=Path(os.environ['OMISSION_ROOT'])
PREVIOUS=Path(os.environ['SAVED_SCREEN_ROOT'])
ids=json.loads((ROOT/'identities.json').read_text());results=[]
for name in ['slope','rank_deficient_saved']:
    request=next(r for r in json.loads((PREVIOUS/'control-requests.json').read_text()) if r['name']==name)
    manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=180
    out=ROOT/'original-requests'/name
    result=measure(ROOT,request['cell'],'fereg','warm',Path(request['data']),Path(request['warm']),out,manifest,source=ROOT/'candidate',identity={'candidate_plugin':ids['candidate_plugin'],'baseline_commit':ids['baseline_commit'],'purpose':'original stopped/planned request; functional evidence outside performance aggregates'})
    row=dict(name=name,status=result['status'],metadata=result['metadata'])
    if result['status']=='pass':row['native_science']=certify(out)
    results.append(row);(ROOT/'original-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(row),flush=True)
    assert result['status']=='pass',name
