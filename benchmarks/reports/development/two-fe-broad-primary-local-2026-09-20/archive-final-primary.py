"""Archive both complete primary blocks, without promoting reference failures."""
from pathlib import Path
import collections, json, shutil, statistics, sys
sys.path.insert(0, str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive, sha, write
from benchmarks.two_fe_dense.run import validate_controller

base=Path('target/two-fe-broad-20260920');root=base/'final-usefulpilot'
out=Path('benchmarks/reports/development/two-fe-broad-primary-local-2026-09-20')
blocks=['primary-final-a','primary-final-b']
assert 'BOTH_PRIMARY_SCIENCE_REVIEWS_COMPLETE' in (root/'primary-science.stdout.txt').read_text()
scores={b:json.loads((root/(b+'-score.json')).read_text()) for b in blocks}
assert all(s['identity_audit_complete'] and s['timing_threshold_met'] and s['planned']==77 and s['repetitions']==5 for s in scores.values())
assert len({s['reference_identity'] for s in scores.values()})==1
for b in ['fresh-holdout-a','fresh-holdout-b']:
    assert json.loads((root/(b+'-score.json')).read_text())['reference_identity']==scores[blocks[0]]['reference_identity']
validate_controller(root/'controller')
labels=[p['label'] for b in blocks for p in json.loads((root/(b+'-progress.json')).read_text())]
assert len(labels)==154
archive(root,out,labels)
for b in blocks:
    for suffix in ['plan','progress','complete','score','graph-oracle','science-audit']:
        shutil.copy(root/(b+'-'+suffix+'.json'),out)
for name in ['primary-dispatch-plan.json','primary-dispatch.stdout.txt','primary-science.stdout.txt']:
    shutil.copy(root/name,out)
shutil.copytree(root/'controller',out/'controller',ignore=shutil.ignore_patterns('__pycache__'))
measurements=json.loads((out/'measurements.json').read_text())
assert len(measurements)==1540 and all(m['status']=='pass' and m['returncode']==0 and m['sentinel'] for m in measurements)
assert all(all(sha(out/'controller'/n)==h for n,h in m['controller'].items()) for m in measurements)
outputs=[]
for label in labels:
    p=root/(label+'-usefulpilot-independent-outputs.json')
    if p.exists():outputs.extend(json.loads(p.read_text()))
assert len(outputs)==80
write(out/'independent-outputs.json',outputs)
native=[m for m in measurements if m['variant']=='usefulpilot']
science_names=['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_certificate.csv','fereg_fit.csv']
rows=[];repeat_checks=[]
for i in range(77):
    cells=[scores[b]['cells'][i] for b in blocks]
    assert cells[0]['cell']==cells[1]['cell']
    assert len({p['input_sha256'] for c in cells for p in c['pairs']})==1
    selected=[m for m in native if m['series'] in [f'{b}-c{i:03d}' for b in blocks]]
    assert len(selected)==10
    exact={n:len({m['scientific_files'][n] for m in selected})==1 for n in science_names}
    sample=len({m['sample_sha256'] for m in selected})==1
    output=len({m.get('outputs_sha256') for m in selected})==1
    assert exact['fereg_status.csv'] and exact['fereg_dof.csv'] and sample
    repeat_checks.append(dict(index=i,calls=10,science_exact=exact,sample_exact=sample,outputs_exact=output))
    rows.append(dict(index=i,cell=cells[0]['cell'],blocks={b:{k:c[k] for k in
        ['median_paired_ratio','candidate_median_seconds','comparator_median_seconds','scientific_comparison','wins']} for b,c in zip(blocks,cells)}))
identity=json.loads((base/'identity-usefulpilot.json').read_text())
assert all(sha(Path(p))==h for p,h in identity['source'].items())
assert sha(base/'usefulpilot/dist/fereg_macos.plugin')==identity['plugin']
oracle_records=[r for b in blocks for r in json.loads((root/(b+'-graph-oracle.json')).read_text())['records']]
native_oracles=[r for r in oracle_records if r['run'].endswith('-usefulpilot')]
reference_oracles=[r for r in oracle_records if r['run'].endswith('-fixest')]
assert len(native_oracles)==360 and all(r['status']=='pass' for r in native_oracles)
groups=collections.defaultdict(list)
for r in rows:groups[r['cell']['group']].append(r)
families={g:dict(cells=len(rs),blocks={b:dict(geomean=statistics.geometric_mean(r['blocks'][b]['median_paired_ratio'] for r in rs),
    worst=max(r['blocks'][b]['median_paired_ratio'] for r in rs)) for b in blocks}) for g,rs in groups.items()}
write(out/'primary-audit.json',dict(status='native_science_and_default_reference_timing_pass',
    source_files_verified=len(identity['source']),calls=1540,pairs=770,cells=77,blocks=2,
    candidate_plugin=identity['plugin'],reference_identity=scores[blocks[0]]['reference_identity'],
    timing_threshold_met=True,worst_by_block={b:scores[b]['worst_ratio'] for b in blocks},
    individual_pair_wins=sum(r['blocks'][b]['wins'] for r in rows for b in blocks),
    native_graph_oracles=360,independent_output_audits=80,
    default_reference_graph_mismatches=sum(r['status']!='pass' for r in reference_oracles),
    native_max_coefficient_error=max(r['coefficients_max_error'] for r in native_oracles),
    reference_max_coefficient_error=max(r['coefficients_max_error'] for r in reference_oracles),
    matched_accuracy_adjudication='separate evidence; default mismatches remain',
    families=families,repeat_checks=repeat_checks,rows=rows))
for name in ['review-primary-science.py','run-final-primary-blocks.py','archive-final-primary.py']:
    shutil.copy(base/name,out)
print('PRIMARY_ARCHIVE_PASS', {b:scores[b]['worst_ratio'] for b in blocks},flush=True)
print('FAMILIES',families,flush=True)
print('EXACT_CELLS',sum(all(r['science_exact'].values()) and r['outputs_exact'] for r in repeat_checks),flush=True)
