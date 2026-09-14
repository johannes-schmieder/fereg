import os
from pathlib import Path
import os,json,copy,hashlib
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import compare,named_runtime
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'diagnostic-built.json').read_text());requests=json.loads((r/'screen-requests.json').read_text());requests.append(next(x for x in json.loads((r/'followup-requests.json').read_text()) if x['name']=='low_fw'));rows=[]
for req in requests:
 if req['name']=='one_fe_complete':continue
 paths={};row={'name':req['name'],'diagnostic':True};assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 for arm in ['baseline','candidate']:
  source=r/('diagnostic-'+arm);assert sha(source/'dist/fereg_macos.plugin')==ids[arm]['plugin'];assert all(sha(source/p)==h for p,h in ids[arm]['inputs'].items());out=r/'diagnostic-runs'/(req['name']+'-'+arm)
  result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,copy.deepcopy(MANIFEST),source=source,trace=True,identity={'artifact':ids[arm]['plugin'],'purpose':'Separate predictor timing; excluded from command performance aggregates'})
  paths[arm]=out;row[arm]=result;print(req['name'],arm,result['status'],result['seconds'],flush=True);assert result['status']=='pass';ref=r/'runs'/(('control-' if req['name']=='low_fw' else 'screen-')+req['name']+'-'+arm)
  for f in ['b.csv','V.csv','feppml_fit.csv','feppml_certificate.csv','feppml_status.csv','feppml_dof.csv']:assert (out/f).read_bytes()==(ref/f).read_bytes(),(arm,f)
  row[arm+'_predictor_lines']=[x for x in (out/'stderr.txt').read_text().splitlines() if x.startswith('FEREG_PHASE|retained_predictor|')];assert row[arm+'_predictor_lines'];row[arm+'_runtime']=named_runtime(out)
 compare(paths);rows.append(row);(r/'diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('PREDICTOR_DIAGNOSTICS_TERMINAL_PASS',flush=True)
