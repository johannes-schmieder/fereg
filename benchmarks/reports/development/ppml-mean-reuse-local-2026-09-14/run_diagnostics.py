import os
from pathlib import Path
import os,json,copy,hashlib
from measure import measure
from run import MANIFEST
from ppml_preparation_compare import compare,named_runtime
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'diagnostic-built.json').read_text());requests=json.loads((r/'screen-requests.json').read_text());rows=[]
for req in requests:
 if req['name']=='pw_cluster_offset':continue
 paths={};row={'name':req['name'],'diagnostic':True};assert all(sha(Path(req[k]))==req[k+'_sha256'] for k in ['data','warm'])
 os.environ['FEREG_LOCAL_POOL']=str(req['cell'].get('pool',10));os.environ['FEREG_LOCAL_BLOCK_WIDTH']=req.get('block_width','1')
 for arm in ['baseline','candidate']:
  source=r/('diagnostic-'+arm);assert sha(source/'dist/fereg_macos.plugin')==ids[arm]['plugin'];assert all(sha(source/p)==h for p,h in ids[arm]['inputs'].items());out=r/'diagnostic-runs'/(req['name']+'-'+arm)
  result=measure(r,req['cell'],'feppml','warm',Path(req['data']),Path(req['warm']),out,copy.deepcopy(MANIFEST),source=source,solver=req.get('solver','auto'),trace=True,identity={'artifact':ids[arm]['plugin'],'purpose':'Separate mean refresh attribution; excluded from command performance aggregates'})
  paths[arm]=out;row[arm]=result;print(req['name'],arm,result['status'],result['seconds'],flush=True);assert result['status']=='pass'
  ref=r/'runs'/('screen-'+req['name']+'-'+arm)
  compare({'baseline':ref,'candidate':out});row[arm+'_original_comparison']=json.loads((out/'exact-science.json').read_text())
  row[arm+'_mean_lines']=[x for x in (out/'stderr.txt').read_text().splitlines() if x.startswith('FEREG_PHASE|mean_refresh|')];assert row[arm+'_mean_lines'];row[arm+'_runtime']=named_runtime(out)
 compare(paths);row['science']=json.loads((paths['candidate']/'exact-science.json').read_text());rows.append(row);(r/'diagnostic-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print('MEAN_DIAGNOSTICS_TERMINAL_PASS',flush=True)
