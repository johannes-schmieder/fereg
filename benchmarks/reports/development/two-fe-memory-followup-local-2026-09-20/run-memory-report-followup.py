"""Capture the existing public memory matrix after the primary science review.

This instruments only post-timing export; no estimator or model is changed.
The twenty-one cells and one-pair repetition count were declared separately.
"""
from pathlib import Path
import difflib, hashlib, json, os, shutil, subprocess, sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense import run, broad

base=Path('target/two-fe-broad-20260920').resolve()
primary=base/'final-usefulpilot'
for block in ['primary-final-a','primary-final-b']:
    science=json.loads((primary/(block+'-science-audit.json')).read_text())
    assert science['status']=='pass' and science['native_graph_oracles']==180
assert '\nBOTH_PRIMARY_BLOCK_ATTEMPTS_COMPLETE\n' in (primary/'primary-dispatch.stdout.txt').read_text()
plan_path=base/'memory-report-followup-plan.json';plan=json.loads(plan_path.read_text())
assert len(plan['cells'])==21 and plan['pairs_per_cell']==1
root=base/'memory-report-followup';assert not root.exists()
root.mkdir();(root/'data').symlink_to(base/'data',target_is_directory=True)
folder=run.stage(root);worker=folder/'stata_worker.do';before=worker.read_text()
instrumentation=root/'instrumentation';instrumentation.mkdir()
shutil.copy(worker,instrumentation/'original-worker.do')
shutil.copy(folder/'source-manifest.json',instrumentation/'original-manifest.json')
after=run.breadth.replace_once(before,'foreach matrix_name in b V fereg_solver fereg_runtime ',
    'foreach matrix_name in b V fereg_solver fereg_runtime fereg_memory ')
after=run.breadth.replace_once(after,"file close `meta'\n",
    '''file write `meta' "memory_method" _tab `"`e(fereg_memory_method)'"' _n
file close `meta'
''')
assert before.split('timer off 92')[0]==after.split('timer off 92')[0]
worker.write_text(after)
(instrumentation/'worker.patch').write_text(''.join(difflib.unified_diff(
    before.splitlines(True),after.splitlines(True),fromfile='original',tofile='memory-export')))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
run.write(folder/'source-manifest.json',run.controller_manifest(folder))
run.write(folder/'memory-export-identity.json',dict(schema='two-fe-memory-export-v1',
    script_sha256=sha(Path(__file__)),plan_sha256=sha(plan_path),
    original_worker_sha256=sha(instrumentation/'original-worker.do'),
    original_manifest_sha256=sha(instrumentation/'original-manifest.json'),
    worker_patch_sha256=sha(instrumentation/'worker.patch'),timed_body_unchanged=True,
    added_matrix='fereg_memory',added_metadata='memory_method'))
run.validate_controller(folder)
for name in ['identity-baseline.json','identity-usefulpilot.json','baseline-provenance.json']:
    shutil.copy(base/name,root/name)
shutil.copy(plan_path,root/'plan.json')
assert os.environ.get('ONE_FE_PROFILE_REPEAT')=='1'
progress=[]
for entry in plan['cells']:
    label=f'memory-report-c{entry["index"]:03d}'
    command=[sys.executable,'benchmarks/two_fe_dense/run.py',str(root),'--artifacts',str(base),
        '--label',label,'--variants',*plan['variants'],'--repeats','1',*broad.arguments(entry['cell'])]
    output=root/(label+'.stdout.txt')
    with output.open('w') as stream:
        process=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
    complete_path=root/(label+'-complete.json')
    complete=json.loads(complete_path.read_text()) if complete_path.exists() else None
    rec=dict(index=entry['index'],cell=entry['cell'],label=label,returncode=process.returncode,
        status=complete['status'] if complete else 'incomplete',stdout_sha256=sha(output))
    progress.append(rec);run.write(root/'progress.json',progress)
    print('MEMORY_REPORT_CELL',entry['index'],rec['returncode'],rec['status'],flush=True)
    assert process.returncode==0 and rec['status']=='pass', rec
    for variant in plan['variants']:
        results=list((root/'runs').glob(label+'-*-'+variant+'/result.json'));assert len(results)==1
        assert (results[0].parent/'fereg_memory.csv').exists()
run.write(root/'complete.json',dict(status='pass',cells=len(progress),calls=2*len(progress),
    plan_sha256=sha(root/'plan.json'),progress_sha256=sha(root/'progress.json')))
print('MEMORY_REPORT_FOLLOWUP_COMPLETE',flush=True)
