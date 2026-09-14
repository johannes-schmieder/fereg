import os
from pathlib import Path
import json,hashlib,shutil,subprocess
r=Path(__file__).resolve().parent;previous=Path(os.environ['GENERATED_PREDICTOR_ROOT']);repo=Path(os.environ['REPOSITORY_ROOT']);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();old=json.loads((previous/'built-identities.json').read_text())
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='25bf610da33721f0914f852d3c5b8488a65cd04f'
assert all(sha(repo/p)==h and sha(previous/'candidate'/p)==h for p,h in old['candidate_inputs'].items())
assert sha(repo/'dist/fereg_macos.plugin')==old['installed_dist']
(r/'baseline').symlink_to(previous/'candidate',target_is_directory=True)
ids={'baseline_commit':'25bf610da33721f0914f852d3c5b8488a65cd04f','baseline_inputs':old['candidate_inputs'],'baseline_plugin':old['candidate_plugin'],'installed_dist':old['installed_dist']}
(r/'baseline-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
src=r/'baseline';dest=r/'diagnostic-baseline';shutil.copytree(src,dest,ignore=shutil.ignore_patterns('dist','target'))
rel='crates/fereg-core/src/ols/prepared_fit.rs';p=dest/rel;s=p.read_text()
a=s.index('fn build_independent_raw_residuals<');b=s.index('\n// Shared upper bound',a);part=s[a:b]
part=part.replace('    let rows = prepared.fit_sample().n_active();','    let probe_started = std::time::Instant::now();\n    let rows = prepared.fit_sample().n_active();',1)
part=part.replace('    residuals.extend_from_slice(response);','    residuals.extend_from_slice(response);\n    let initialize_seconds = probe_started.elapsed().as_secs_f64();\n    let accumulate_started = std::time::Instant::now();',1)
part=part.replace('    Ok(residuals)','''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|ols_raw_residual|mode=fast|rows={rows}|generated={}|initialize={initialize_seconds:.9}|accumulate={:.9}|total={:.9}", prepared.fit_model().columns.len(), accumulate_started.elapsed().as_secs_f64(), probe_started.elapsed().as_secs_f64());
    }
    Ok(residuals)''',1)
s=s[:a]+part+s[b:]
needle='''    let mut raw_residuals = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS final residual",
    )?;''';assert s.count(needle)==1
s=s.replace(needle,'    let low_raw_started = std::time::Instant::now();\n'+needle+'\n    let low_accumulate_started = std::time::Instant::now();',1)
a=s.index('    let low_raw_started');b=s.index('    let bread = finalize_reported_bread_and_intercept(',a)
s=s[:b]+'''    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|ols_raw_residual|mode=low|rows={rows}|generated={}|accumulate={:.9}|total={:.9}", prepared.fit_model().columns.len(), low_accumulate_started.elapsed().as_secs_f64(), low_raw_started.elapsed().as_secs_f64());
    }
'''+s[b:]
p.write_text(s);diff=subprocess.run(['diff','-u','--label','a/'+rel,'--label','b/'+rel,str(src/rel),str(p)],capture_output=True);assert diff.returncode==1;(r/'diagnostic-baseline.patch').write_bytes(diff.stdout)
(r/'diagnostic-identities.json').write_text(json.dumps({'baseline':{'inputs':{p:sha(dest/p) for p in ids['baseline_inputs']},'original_plugin':ids['baseline_plugin'],'source_patch_sha256':sha(r/'diagnostic-baseline.patch')}},indent=2)+'\n')
oldroot=Path(os.environ['GENERATED_RHS_ROOT']);reqs=json.loads((oldroot/'requests.json').read_text())[:2]
for q in reqs:
 for k in ['data','warm']:assert sha(Path(q[k]))==q[k+'_sha256']
(r/'requests.json').write_text(json.dumps(reqs,indent=2)+'\n');shutil.copytree(previous/'harness',r/'harness',ignore=shutil.ignore_patterns('__pycache__'));shutil.copyfile(previous/'harness-identities.json',r/'harness-identities.json')
print('OLS_RESIDUAL_DIAGNOSTIC_PREPARED')
