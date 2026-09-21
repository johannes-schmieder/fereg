"""Bind final tighter-reference attempts to the unchanged headline cells."""
from pathlib import Path
import hashlib, json, shutil, sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive, sha, write
from benchmarks.two_fe_dense.graph_oracle import audit as graph_audit
from benchmarks.two_fe_dense.run import validate_controller

base=Path('target/two-fe-broad-20260920')
first=base/'final-reference-precision';second=base/'rp2';primary=base/'final-usefulpilot'
out=Path('benchmarks/reports/development/two-fe-final-reference-accuracy-local-2026-09-20')
assert 'ALL_FINAL_REFERENCE_PRECISION_ATTEMPTS_COMPLETE' in (base/'final-reference-precision-short.stdout.txt').read_text()
plan=json.loads((first/'plan.json').read_text())
assert plan==json.loads((second/'plan.json').read_text()) and len(plan['cells'])==28
initial=json.loads((first/'progress.json').read_text());continuation=json.loads((second/'progress.json').read_text())
assert len(initial)==2 and len(continuation)==27
assert initial[1]['application_status']==dict(usefulpilot='worker_failure',fixest='missing')
assert [p['index'] for p in continuation]==list(range(1,28))
archive(first,out,[p['label'] for p in initial])
archive(second,out/'continuation',[p['label'] for p in continuation])
for root,dest in [(first,out),(second,out/'continuation')]:
    validate_controller(root/'controller');shutil.copytree(root/'controller',dest/'controller',ignore=shutil.ignore_patterns('__pycache__'))
    for name in ['plan.json','progress.json','dispatch-identity.json']:
        shutil.copy(root/name,dest)
    if (root/'complete.json').exists():shutil.copy(root/'complete.json',dest)
    for m in json.loads((dest/'measurements.json').read_text()):
        assert all(sha(dest/'controller'/n)==h for n,h in m['controller'].items())
rows=[];graph_paths=[]
ref_id=json.loads((primary/'primary-final-a-score.json').read_text())['reference_identity']
science=['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_certificate.csv','fereg_fit.csv']
for attempt in [initial[0],*continuation]:
    i=attempt['index'];entry=plan['cells'][i];root=first if i==0 else second
    paths={v:next((root/'runs').glob(attempt['label']+'-*-'+v+'/result.json')) for v in ['usefulpilot','fixest']}
    native,ref=[json.loads(paths[v].read_text()) for v in ['usefulpilot','fixest']]
    assert native['status']=='pass' and ref['status'] in ['pass','nonconvergence']
    original_paths={v:next((primary/'runs').glob(entry['block']+f'-c{entry["index"]:03d}-*-r1-'+v+'/result.json')) for v in paths}
    orig=json.loads(original_paths['usefulpilot'].read_text())
    assert native['input_sha256']==ref['input_sha256']==orig['input_sha256']
    assert native['sample_sha256']==orig['sample_sha256']
    exact={n:sha(paths['usefulpilot'].parent/n)==sha(original_paths['usefulpilot'].parent/n) for n in science}
    assert all(exact.values())
    output_exact=None
    if (paths['usefulpilot'].parent/'outputs.bin').exists():
        output_exact=sha(paths['usefulpilot'].parent/'outputs.bin')==sha(original_paths['usefulpilot'].parent/'outputs.bin');assert output_exact
    fp=ref['measurement_fingerprint']
    fingerprint=hashlib.sha256(json.dumps({k:fp[k] for k in ['fixest_archive_sha256','fixest_library']},sort_keys=True).encode()).hexdigest()
    assert fingerprint==ref_id
    assert native['metadata']['command']==orig['metadata']['command']
    if ref['status']=='pass':assert attempt['status']=='pass' and attempt['returncode']==0
    else:assert attempt['status']=='incomplete' and attempt['returncode']!=0
    if entry['cell']['group'] in ['other-graphs','fresh-weak']:
        graph_paths.append(paths['usefulpilot'])
        if ref['status']=='pass':graph_paths.append(paths['fixest'])
    rows.append(dict(index=i,source=entry,comparison=attempt['status'],reference_status=ref['status'],
        native_science_identical_to_headline=exact,native_outputs_identical_to_headline=output_exact,
        native_seconds=native['seconds'],reference_seconds=ref['seconds'],
        input_sha256=native['input_sha256'],result_sha256={v:sha(p) for v,p in paths.items()}))
graph_audit(graph_paths,out/'graph-oracle.json')
checks=json.loads((out/'graph-oracle.json').read_text())['records']
assert all(r['status']=='pass' for r in checks)
failed=next((first/'runs').glob(initial[1]['label']+'-*/result.json'))
failure=json.loads(failed.read_text());assert failure['process_returncode']==-5 and failure['failure_stage']=='before_target_invocation'
assert failure['metadata']=={} and not failure['sentinel']
write(out/'accuracy-audit.json',dict(status='adjudicated_with_explicit_reference_nonconvergence',
    distinct_cells=28,attempted_calls=57,successful_native_calls=28,startup_failure_preserved=True,
    reference_successes=sum(r['reference_status']=='pass' for r in rows),
    reference_nonconvergence=sum(r['reference_status']=='nonconvergence' for r in rows),
    graph_oracle_checks=len(checks),all_native_science_identical_to_headline=True,
    all_requested_native_outputs_identical_to_headline=True,reference_identity=ref_id,
    tighter_reference_times_replace_headlines=False,rows=rows))
for name in ['run-final-reference-precision.py','run-final-reference-precision-short.py','archive-final-reference.py','precision-startup-failure.json']:
    shutil.copy(base/name,out)
print('FINAL_REFERENCE_ACCURACY_AUDIT_PASS',len(checks),'graph checks',sum(r['reference_status']=='nonconvergence' for r in rows),'nonconverged references',flush=True)
