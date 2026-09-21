"""Run only after BOTH primary timing blocks are terminal."""
from pathlib import Path
import json, sys
sys.path.insert(0, str(Path.cwd()))
from benchmarks.two_fe_dense.graph_oracle import audit as graph_audit
from benchmarks.two_fe_dense.audit_outputs import audit as output_audit
from benchmarks.two_fe_dense.score import score
from benchmarks.two_fe_dense.broad import expand
from benchmarks.two_fe_dense.archive import write, sha

root = Path('target/two-fe-broad-20260920/final-usefulpilot')
assert '\nBOTH_PRIMARY_BLOCK_ATTEMPTS_COMPLETE\n' in (root/'primary-dispatch.stdout.txt').read_text()
spec = json.loads(Path('benchmarks/two_fe_dense/broad_milestone.json').read_text())
blocks = ['primary-final-a', 'primary-final-b']
for label in blocks:
    output = root/(label+'-score.json')
    assert not output.exists(), 'preserve earlier scoring attempts'
    result = score(root, label)
    write(output, result)
    assert result['identity_audit_complete'] and result['planned']==77
    assert [c['cell'] for c in result['cells']] == expand(spec, 'primary')
    print('PRIMARY_SCORE',label,result['wins'],result['ties'],result['losses'],result['worst_ratio'],flush=True)
    progress = json.loads((root/(label+'-progress.json')).read_text())
    graph_paths=[]; output_runs=0
    for p in progress:
        assert p['process_returncode']==0
        if p['cell']['group']!='other-graphs':
            assert p['comparison_status'] in ('pass','reference_output_mismatch'), p
        native=list((root/'runs').glob(p['label']+'-*-usefulpilot/result.json'))
        assert len(native)==5
        cell=json.loads(native[0].read_text())['cell']
        if cell['outputs']!='none':
            destination=root/(p['label']+'-usefulpilot-independent-outputs.json')
            assert not destination.exists()
            output_audit(root,p['label'],'usefulpilot')
            checks=json.loads(destination.read_text());assert len(checks)==5
            output_runs+=len(checks)
        if p['cell']['group']=='other-graphs':
            # Check every repetition, including any nondeterministic reference
            # result; the oracle caches only by canonical input and weight kind.
            for rep in range(1,6):
                for variant in ['usefulpilot','fixest']:
                    matches=list((root/'runs').glob(p['label']+f'-*-r{rep}-{variant}/result.json'))
                    assert len(matches)==1
                    graph_paths.extend(matches)
    assert len(graph_paths)==360 and output_runs==40
    graph_audit(graph_paths,root/(label+'-graph-oracle.json'))
    records=json.loads((root/(label+'-graph-oracle.json')).read_text())['records']
    native=[r for r in records if r['run'].endswith('-usefulpilot')]
    assert len(native)==180 and all(r['status']=='pass' for r in native)
    write(root/(label+'-science-audit.json'),dict(status='pass',native_graph_oracles=len(native),
        independent_output_runs=output_runs,graph_receipt_sha256=sha(root/(label+'-graph-oracle.json')),
        default_reference_graph_disagreements=[r for r in records if not r['run'].endswith('-usefulpilot') and r['status']!='pass'],
        timing_threshold_met=result['timing_threshold_met'],score_sha256=sha(output)))
    print('PRIMARY_NATIVE_SCIENCE_PASS',label,'graph checks',len(native),'output checks',output_runs,flush=True)
print('BOTH_PRIMARY_SCIENCE_REVIEWS_COMPLETE',flush=True)
