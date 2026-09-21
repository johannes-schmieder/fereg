"""Separate accuracy diagnostics; never replace default-reference timing."""
from pathlib import Path
import json, os, shutil, subprocess, sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense import run, broad
from benchmarks.two_fe_dense.archive import sha, write

base=Path('target/two-fe-broad-20260920').resolve()
assert 'MEMORY_REPORT_FOLLOWUP_COMPLETE' in (base/'memory-report-followup.stdout.txt').read_text()
plan=json.loads((base/'final-reference-precision-plan.json').read_text())
assert len(plan['cells'])==28 and plan['pairs_per_cell']==1
root=base/'final-reference-precision';assert not root.exists()
root.mkdir();(root/'data').symlink_to(base/'data',target_is_directory=True)
run.stage(root)
shutil.copy(base/'identity-usefulpilot.json',root)
shutil.copy(base/'final-reference-precision-plan.json',root/'plan.json')
write(root/'dispatch-identity.json',dict(script_sha256=sha(Path(__file__)),plan_sha256=sha(root/'plan.json'),
    controller_manifest_sha256=sha(root/'controller/source-manifest.json')))
assert os.environ.get('ONE_FE_PROFILE_REPEAT')=='1'
progress=[]
for i,entry in enumerate(plan['cells']):
    label=f'final-precision-c{i:03d}'
    command=[sys.executable,'benchmarks/two_fe_dense/run.py',str(root),'--artifacts',str(base),
        '--label',label,'--variants',*plan['variants'],'--repeats','1',*broad.arguments(entry['cell']),
        '--reference-precision','--reference-tolerance',str(plan['reference_tolerance']),
        '--record-reference-output-disagreement','--record-reference-science-disagreement']
    output=root/(label+'.stdout.txt')
    with output.open('w') as stream:
        process=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
    results={}
    for variant in plan['variants']:
        paths=list((root/'runs').glob(label+'-*-'+variant+'/result.json'))
        results[variant]=json.loads(paths[0].read_text()) if len(paths)==1 else None
    done=root/(label+'-complete.json')
    rec=dict(index=i,source=entry,label=label,returncode=process.returncode,
        status=json.loads(done.read_text())['status'] if done.exists() else 'incomplete',
        application_status={v:r['status'] if r else 'missing' for v,r in results.items()},stdout_sha256=sha(output))
    progress.append(rec);write(root/'progress.json',progress)
    print('REFERENCE_PRECISION_CELL',i,rec['returncode'],rec['status'],rec['application_status'],flush=True)
    native=results['usefulpilot'];reference=results['fixest']
    assert native and native['status']=='pass' and native['process_returncode']==0 and native['sentinel'],rec
    assert reference and reference['status'] in ['pass','nonconvergence'],rec
    assert native['input_sha256']==reference['input_sha256']
    assert native['cell']==reference['cell']
    assert native['cell']['fixest_tolerance']==native['cell']['fixest_recovery_tolerance']==1e-11
    prior=list((base/'final-usefulpilot/runs').glob(entry['block']+f'-c{entry["index"]:03d}-*-r1-usefulpilot/result.json'))
    assert len(prior)==1
    original=json.loads(prior[0].read_text())
    assert native['input_sha256']==original['input_sha256']
    assert native['metadata']['command']==original['metadata']['command']
    if reference['status']=='pass':assert process.returncode==0,rec
    else:assert process.returncode!=0 and rec['status']=='incomplete',rec
write(root/'complete.json',dict(status='all_accuracy_attempts_recorded',cells=len(progress),calls=2*len(progress),
    plan_sha256=sha(root/'plan.json'),progress_sha256=sha(root/'progress.json')))
print('ALL_FINAL_REFERENCE_PRECISION_ATTEMPTS_COMPLETE',flush=True)
