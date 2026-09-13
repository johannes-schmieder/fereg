import os
from pathlib import Path
import os,json,hashlib,copy,importlib.util
from run import MANIFEST
from final_compare import certify,baseline_compare
root=Path(os.environ['SAVED_SCREEN_ROOT']);repo=Path(os.environ['REPOSITORY_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=(repo/'benchmarks/local_fixest_gap/measure.py').read_text();needle='    environment.update(thread_environment(threads))'
assert source.count(needle)==1
source=source.replace(needle,needle+'\n    environment["FEREG_MEMORY_LIMIT_BYTES"] = os.environ["FEREG_MEMORY_LIMIT_BYTES"]')
(root/'cap_measure.py').write_text(source)
spec=importlib.util.spec_from_file_location('cap_measure',root/'cap_measure.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
ids=json.loads((root/'built-identities.json').read_text());validation=json.loads((root/'validation-results.json').read_text());assert len(validation)==3 and all(x['sentinel'] for x in validation)
assert '5 passed; 0 failed' in (root/'guard-plugin-tests.log').read_text()
results=[]
for name,bound in [('unit_saved',827753800),('pw_partial',831179336)]:
 reference=Path(os.path.join(os.environ['RECOVERY_ROOT'], 'cost-runs'))/(name+'-candidate')
 req=json.loads((reference/'request.json').read_text())
 for offset in [0,-1]:
  cap=bound+536870912+offset;os.environ['FEREG_MEMORY_LIMIT_BYTES']=str(cap)
  output=root/'initial-caps'/f'{name}-{offset}'
  settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
  if (output/'result.json').exists():
   result=json.loads((output/'result.json').read_text())
   assert result['measurement_fingerprint']['available_cap']==cap
   assert result['measurement_fingerprint']['plugin']==ids['candidate_plugin']
  else:
   result=module.measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),output,settings,source=root/'candidate',identity={'plugin':ids['candidate_plugin'],'protocol':ids['protocol'],'cap_harness':sha(root/'cap_measure.py'),'available_cap':cap,'purpose':'initial joint admission; excluded from ordinary timings'})
  rc=int(result['metadata']['rc']);assert rc in [0,909]
  row={'name':name,'offset':offset,'available_cap':cap,'usable':bound+offset,'rc':rc,'status':result['status'],'metadata':result['metadata']}
  if offset==0:assert rc==0,result
  if rc==0:
   row['native_science']=certify(output);row['comparison']=baseline_compare(output,reference)
   assert result['metadata']['fereg_native_allocation_bytes']<=bound+offset
  else:
   txt='\n'.join(p.read_text(errors='replace') for p in output.glob('*.log'))
   row['error_lines']=[line for line in txt.splitlines() if any(s in line for s in ['memory plan','does not fit','r(909)','SYSTEMATIC_WORKER_COMPLETE rc=909'])]
   assert row['error_lines']
  results.append(row);(root/'initial-cap-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({k:v for k,v in row.items() if k not in ['metadata','native_science']}),flush=True)
