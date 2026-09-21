"""Sequential immutable primary blocks; stop on application/inventory failures."""
from pathlib import Path
import hashlib, json, os, subprocess, sys

repo=Path.cwd();base=repo/'target/two-fe-broad-20260920';root=base/'final-usefulpilot'
manifest=repo/'benchmarks/two_fe_dense/broad_milestone.json'
controller=repo/'benchmarks/two_fe_dense/broad.py'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
plan=dict(schema='two-fe-final-primary-dispatch-v1',manifest_sha256=sha(manifest),
    controller_sha256=sha(controller),runner_sha256=sha(repo/'benchmarks/two_fe_dense/run.py'),
    candidate='usefulpilot',candidate_plugin=json.loads((base/'identity-usefulpilot.json').read_text())['plugin'],
    labels=['primary-final-a','primary-final-b'],phases=['block-a','block-b'],
    per_block_cells=77,pairs_per_cell=5,command_repetitions=1)
destination=root/'primary-dispatch-plan.json'
assert not destination.exists(), 'inspect any previous dispatch before retrying'
destination.write_text(json.dumps(plan,indent=2,sort_keys=True)+'\n')
assert os.environ.get('ONE_FE_PROFILE_REPEAT')=='1'
for label,phase in zip(plan['labels'],plan['phases']):
    assert sha(manifest)==plan['manifest_sha256'] and sha(controller)==plan['controller_sha256']
    assert sha(repo/'benchmarks/two_fe_dense/run.py')==plan['runner_sha256']
    output=root/(label+'.stdout.txt')
    assert not output.exists()
    command=[sys.executable,str(controller),str(root),'--artifacts',str(base),'--manifest',str(manifest),
        '--candidate','usefulpilot','--label',label,'--phase',phase]
    print('PRIMARY_BLOCK_START',label,flush=True)
    with output.open('w') as stream:
        process=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
    complete=json.loads((root/(label+'-complete.json')).read_text())
    assert process.returncode==0 and complete['attempted']==complete['planned']==77
    assert complete['process_failures']==0, complete
    print('PRIMARY_BLOCK_ATTEMPTS_COMPLETE',label,flush=True)
print('BOTH_PRIMARY_BLOCK_ATTEMPTS_COMPLETE',flush=True)
