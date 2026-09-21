from pathlib import Path
import json, sys
sys.path.insert(0, str(Path.cwd()))
from benchmarks.two_fe_dense.graph_oracle import audit as graph_audit
from benchmarks.two_fe_dense.audit_outputs import audit as output_audit
from benchmarks.two_fe_dense.score import score
from benchmarks.two_fe_dense.archive import write

root = Path('target/two-fe-broad-20260920/final-usefulpilot')
label = sys.argv[1]
result = score(root, label)
write(root/(label+'-score.json'), result)
print('HOLDOUT_SCORE', result['planned'], result['wins'], result['ties'], result['losses'], result['worst_ratio'], flush=True)
progress = json.loads((root/(label+'-progress.json')).read_text())
paths=[]
for p in progress:
    assert p['process_returncode']==0
    if p['cell']['group'] in ['fresh-well-connected','fresh-weak']:
        for variant in ['usefulpilot','fixest']:
            matches=list((root/'runs').glob(p['label']+'-*-r1-'+variant+'/result.json'))
            assert len(matches)==1
            paths.extend(matches)
    if p['cell']['case']=='iid_outputs':
        output_audit(root,p['label'],'usefulpilot')
        print('OUTPUT_AUDIT',p['label'],'pass',flush=True)
graph_audit(paths,root/(label+'-graph-oracle.json'))
records=json.loads((root/(label+'-graph-oracle.json')).read_text())['records']
assert len(records)==18 and all(p['status']=='pass' for p in records if p['run'].endswith('-usefulpilot'))
print('FRESH_HOLDOUT_NATIVE_SCIENCE_PASS',flush=True)
