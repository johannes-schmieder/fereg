from pathlib import Path
import json, shutil, sys, statistics
sys.path.insert(0, str(Path.cwd()))
from benchmarks.two_fe_dense.archive import archive, sha, write

base=Path('target/two-fe-broad-20260920');root=base/'final-usefulpilot'
out=Path('benchmarks/reports/development/two-fe-fresh-holdouts-local-2026-09-20')
blocks=['fresh-holdout-a','fresh-holdout-b']
scores={name:json.loads((root/(name+'-score.json')).read_text()) for name in blocks}
assert all(s['identity_audit_complete'] and s['timing_threshold_met'] and s['planned']==16 and s['repetitions']==5 for s in scores.values())
labels=[p['label'] for name in blocks for p in json.loads((root/(name+'-progress.json')).read_text())]
assert len(labels)==32
archive(root,out,labels)
for name in blocks:
    for suffix in ['plan','progress','complete','score','graph-oracle']:
        shutil.copy(root/(name+'-'+suffix+'.json'),out)
    shutil.copy(root/('review-'+name.replace('fresh-','')+'.stdout.txt'),out)
controller=out/'controller';controller.mkdir()
for p in (root/'controller').iterdir():
    if p.is_file():shutil.copy(p,controller)
measurements=json.loads((out/'measurements.json').read_text())
assert len(measurements)==320 and all(m['status']=='pass' and m['returncode']==0 and m['sentinel'] for m in measurements)
assert all(all(sha(controller/n)==h for n,h in m['controller'].items()) for m in measurements)
outputs=[]
for label in labels:
    p=root/(label+'-usefulpilot-independent-outputs.json')
    if p.exists():outputs.extend(json.loads(p.read_text()))
assert len(outputs)==20
write(out/'independent-outputs.json',outputs)
native_oracles=[];reference_oracles=[]
for name in blocks:
    records=json.loads((root/(name+'-graph-oracle.json')).read_text())['records']
    native_oracles.extend(r for r in records if r['run'].endswith('-usefulpilot'))
    reference_oracles.extend(r for r in records if r['run'].endswith('-fixest'))
assert len(native_oracles)==18 and all(r['status']=='pass' for r in native_oracles)
native=[m for m in measurements if m['variant']=='usefulpilot']
science_names=['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_certificate.csv','fereg_fit.csv']
repeat_checks=[];cell_rows=[]
for index in range(16):
    cells=[scores[n]['cells'][index] for n in blocks]
    assert cells[0]['cell']==cells[1]['cell']
    assert {p['input_sha256'] for c in cells for p in c['pairs']}.__len__()==1
    selected=[m for m in native if m['series'] in [f'{n}-c{index:03d}' for n in blocks]]
    assert len(selected)==10
    exact={name:len({m['scientific_files'][name] for m in selected})==1 for name in science_names}
    sample_exact=len({m['sample_sha256'] for m in selected})==1
    output_exact=len({m.get('outputs_sha256') for m in selected})==1
    assert all(exact.values()) and sample_exact and output_exact
    repeat_checks.append(dict(index=index,cell=cells[0]['cell'],calls=10,science_files_exact=exact,
                              sample_exact=sample_exact,outputs_exact=output_exact))
    cell_rows.append(dict(index=index,cell=cells[0]['cell'],blocks={n:{k:c[k] for k in
        ['median_paired_ratio','candidate_median_seconds','comparator_median_seconds','scientific_comparison','wins']}
        for n,c in zip(blocks,cells)}))
identity=json.loads((base/'identity-usefulpilot.json').read_text())
assert all(sha(Path(p))==h for p,h in identity['source'].items())
assert sha(base/'usefulpilot/dist/fereg_macos.plugin')==identity['plugin']
write(out/'holdout-audit.json',dict(status='pass',calls=320,pairs=160,planned_cells=16,blocks=2,
    timing_acceptance=all(s['timing_threshold_met'] for s in scores.values()),
    worst_median_ratio_by_block={n:s['worst_ratio'] for n,s in scores.items()},
    per_call_wins=sum(s['wins'] for c in cell_rows for s in c['blocks'].values()),
    native_scientific_oracles=len(native_oracles),independent_output_audits=len(outputs),
    default_reference_graph_mismatches=[r['run'] for r in reference_oracles if r['status']!='pass'],
    source_files_verified=len(identity['source']),native_repeat_checks=repeat_checks,cells=cell_rows,
    primary_77_blocks_complete=False))
shutil.copy(base/'review-holdout-science.py',out)
shutil.copy(Path(__file__),out)
print('FRESH_HOLDOUT_ARCHIVE_PASS',[(n,s['worst_ratio']) for n,s in scores.items()],flush=True)
for c in cell_rows:print(c['index'],c['cell']['design'],c['cell']['k'],c['cell']['weight_kind'],c['cell']['case'],[(n,round(x['median_paired_ratio'],6)) for n,x in c['blocks'].items()],flush=True)
