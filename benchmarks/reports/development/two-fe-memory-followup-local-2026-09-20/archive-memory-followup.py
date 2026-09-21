"""Audit existing memory forecasts and preserve their strict-proof gaps."""
from pathlib import Path
import csv, json, shutil, sys
sys.path.insert(0,str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive, sha, write
from benchmarks.two_fe_dense.run import validate_controller

base=Path('target/two-fe-broad-20260920');root=base/'memory-report-followup'
old=Path('benchmarks/reports/development/two-fe-broad-native-controls-local-2026-09-20')
out=Path('benchmarks/reports/development/two-fe-memory-followup-local-2026-09-20')
assert 'MEMORY_REPORT_FOLLOWUP_COMPLETE' in (base/'memory-report-followup.stdout.txt').read_text()
plan=json.loads((root/'plan.json').read_text())
for p,h in plan['provenance'].items():assert sha(old/p)==h
complete=json.loads((root/'complete.json').read_text());assert complete['status']=='pass' and complete['calls']==42
progress=json.loads((root/'progress.json').read_text());assert len(progress)==21
validate_controller(root/'controller')
before=(root/'instrumentation/original-worker.do').read_text();after=(root/'controller/stata_worker.do').read_text()
assert before.split('timer off 92')[0]==after.split('timer off 92')[0]
disposition=json.loads((old/'control-disposition.json').read_text())
prior={r['result_sha256']:r for d in [old,old/'repair'] for r in json.loads((d/'measurements.json').read_text())}
science=['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_certificate.csv','fereg_fit.csv']
records=[]
for entry,attempt in zip(plan['cells'],progress):
    assert entry['index']==attempt['index'] and entry['cell']==attempt['cell'] and attempt['status']=='pass'
    original=disposition['cells'][entry['index']];assert original['cell']==entry['cell']
    arms={}
    for variant,role in [('baseline','baseline'),('usefulpilot','candidate')]:
        paths=list((root/'runs').glob(attempt['label']+'-*-'+variant+'/result.json'));assert len(paths)==1
        p=paths[0];d=p.parent;r=json.loads(p.read_text())
        assert r['status']=='pass' and r['process_returncode']==0 and r['sentinel']
        assert all(sha(root/'controller'/n)==h for n,h in r['measurement_fingerprint']['controller'].items())
        raw=list(csv.DictReader((d/'fereg_memory.csv').open()))
        cells={(int(x['row']),int(x['column'])):float(x['value']) for x in raw}
        assert len(raw)==22 and set(cells)=={(i,j) for i in range(1,12) for j in [1,2]}
        assert all(cells[i,1] in [0,1] for i in range(1,12))
        values={i:int(cells[i,2]) if cells[i,1] else None for i in range(1,12)}
        assert all(v is None or v==cells[i,2] for i,v in values.items())
        assert values[1]==2 and values[2] in [1,2] and values[6]>0
        method=r['metadata']['memory_method']
        assert method==('phase-capacity-calibrated-v2' if values[2]==2 else 'phase-capacity-partial-v2')
        runtime={int(x['row']):float(x['value']) for x in csv.DictReader((d/'fereg_runtime.csv').open()) if x['column']=='2'}
        key='comparator_result_sha256' if role=='baseline' else 'candidate_result_sha256'
        previous=prior[original['pairs'][0][key]]
        assert r['input_sha256']==previous['input_sha256'] and r['sample_sha256']==previous['sample_sha256']
        assert r['measurement_fingerprint']['native']['plugin']==previous['plugin']
        exact={n:sha(d/n)==previous['scientific_files'][n] for n in science}
        assert all(exact.values())
        output_exact=None
        if (d/'outputs.bin').exists():
            output_exact=sha(d/'outputs.bin')==previous['outputs_sha256'];assert output_exact
        observation=entry['prior_observation']
        assert runtime[25]==observation['estimated_selected_'+role]
        arms[role]=dict(run=d.name,result_sha256=sha(p),memory_schema=values[1],report_status=values[2],
            method=method,defined={i:bool(cells[i,1]) for i in range(1,12)},report_values=values,
            strict_and_coverage_gap_bits=values[11],whole_action_forecast_bytes=values[6],
            observed_native_peak_bytes=int(runtime[13]),observed_over_forecast=runtime[13]/values[6],
            early_selected_estimate_bytes=int(runtime[25]),early_estimate_ratio=runtime[26],
            delta_from_prior_observation=int(runtime[13]-observation['peak_'+role]),
            previous_six_science_files_exact=exact,previous_output_exact=output_exact,
            input_sha256=r['input_sha256'])
    delta=arms['candidate']['observed_native_peak_bytes']-arms['baseline']['observed_native_peak_bytes']
    assert delta==entry['prior_observation']['peak_candidate']-entry['prior_observation']['peak_baseline']
    records.append(dict(index=entry['index'],cell=entry['cell'],reasons=entry['reasons'],arms=arms,
        peak_delta_candidate_minus_baseline=delta,prior_observation=entry['prior_observation']))
archive(root,out,[p['label'] for p in progress])
shutil.copytree(root/'controller',out/'controller',ignore=shutil.ignore_patterns('__pycache__'));shutil.copytree(root/'instrumentation',out/'instrumentation')
for name in ['plan.json','progress.json','complete.json']:shutil.copy(root/name,out)
for name in ['run-memory-report-followup.py','archive-memory-followup.py']:shutil.copy(base/name,out)
write(out/'memory-audit.json',dict(status='pass_with_explicit_strict_proof_gaps',cells=21,calls=42,
    all_prior_science_files_exact=True,all_requested_prior_outputs_exact=True,
    peak_deltas_match_prior=True,max_observed_over_whole_action_forecast=max(a['observed_over_forecast'] for r in records for a in r['arms'].values()),
    gap_bits=sorted({a['strict_and_coverage_gap_bits'] for r in records for a in r['arms'].values()}),
    no_estimator_or_model_change=True,complete_memory_proof=False,records=records))
print('MEMORY_FOLLOWUP_AUDIT_PASS',max(a['observed_over_forecast'] for r in records for a in r['arms'].values()),flush=True)
