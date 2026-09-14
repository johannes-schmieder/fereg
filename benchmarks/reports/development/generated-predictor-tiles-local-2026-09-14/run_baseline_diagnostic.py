import os
from pathlib import Path
import os,json,copy,hashlib,sys
r=Path(__file__).resolve().parent;sys.path.insert(0,str(r/'harness'))
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import named_runtime
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'diagnostic-built.json').read_text());req=json.loads((r/'requests.json').read_text())[0];source=r/'diagnostic-baseline';assert all(sha(source/p)==h for p,h in ids['baseline']['inputs'].items());assert sha(source/'dist/fereg_macos.plugin')==ids['baseline']['plugin'];out=r/'baseline-diagnostic';manifest=copy.deepcopy(MANIFEST);manifest['measurement']['command_timeout_seconds']=120
result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,manifest,source=source,trace=True,identity={'artifact':ids['baseline']['plugin'],'purpose':'Original generated PPML predictor accumulation cost; diagnostic only'})
row={'result':result};(r/'baseline-diagnostic.json').write_text(json.dumps(row,indent=2)+'\n');print(result['status'],result['seconds'],flush=True);assert result['status']=='pass';ref=Path(os.path.join(os.environ['GENERATED_RHS_ROOT'], 'screen-runs/ppml_fw-baseline'));exports=['b','V','feppml_fit','feppml_certificate','feppml_status','feppml_dof','feppml_fe','feppml_cluster','feppml_subset','feppml_separation','feppml_inference','feppml_drop_counts','feppml_receipt'];row['exact_science']={name:(out/(name+'.csv')).read_bytes()==(ref/(name+'.csv')).read_bytes() for name in exports};assert all(row['exact_science'].values());row['predictor_lines']=[line for line in (out/'stderr.txt').read_text().splitlines() if line.startswith('FEREG_PHASE|retained_predictor|')];assert row['predictor_lines'];row['runtime']=named_runtime(out);(r/'baseline-diagnostic.json').write_text(json.dumps(row,indent=2)+'\n');print('\n'.join(row['predictor_lines']),flush=True);print('BASELINE_GENERATED_PREDICTOR_DIAGNOSTIC_PASS',flush=True)
