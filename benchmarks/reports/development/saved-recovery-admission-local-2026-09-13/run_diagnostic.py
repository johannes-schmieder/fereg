from pathlib import Path
import os
import json, copy, hashlib
from measure import measure
from run import MANIFEST
from final_compare import certify, baseline_compare
root=Path(os.environ['EXPERIMENT_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((root/'v2-source-manifest.json').read_text())
diffs=[p for p,h in manifest['inputs'].items() if sha(root/'diagnostic-v2'/p)!=h]
assert diffs==['crates/fereg-plugin/src/estimate.rs'],diffs
log=(root/'build-diagnostic-v2.log').read_text();assert log.count('Compiling fereg-plugin')==2 and 'actual-plugin ABI/self-test harness: PASS' in log
request=json.loads(Path(os.path.join(os.environ['SAVED_SCHUR_REPAIRED_ROOT'], 'runs/screen-pw_partial-s104729-candidate/request.json')).read_text())
assert sha(Path(request['data']))=='eecf8666668f96734ee06c52310b9920f29c65fc9cce86fd3707be22399fc7d0'
ids={'plugin_sha256':sha(root/'diagnostic-v2/dist/fereg_macos.plugin'),'changed_inputs':{p:sha(root/'diagnostic-v2'/p) for p in diffs},'diagnostic_patch_sha256':sha(root/'diagnostic-v2.patch'),'kind':'allocation/timing diagnostic; excluded from performance aggregates'}
(root/'diagnostic-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
output=root/'diagnostic-pw-original'; settings=copy.deepcopy(MANIFEST);settings['measurement']['command_timeout_seconds']=180
result=measure(root,request['cell'],'fereg','warm',Path(request['data']),Path(request['warm']),output,settings,source=root/'diagnostic-v2',identity=ids)
print(json.dumps({'status':result['status'],'seconds_diagnostic':result['seconds'],'metadata':result['metadata']}),flush=True)
assert result['status']=='pass'
(output/'native-science.json').write_text(json.dumps(certify(output),indent=2)+'\n')
ref=Path(os.path.join(os.environ['SAVED_SCHUR_REPAIRED_ROOT'], 'runs/screen-pw_partial-s104729-candidate'))
comparison=baseline_compare(output,ref)
for file in ['b.csv','V.csv','fereg_status.csv','fereg_dof.csv','fereg_fe.csv','fereg_cluster.csv','fereg_subset.csv','saved_outputs.csv']:
 assert (output/file).read_bytes()==(ref/file).read_bytes(),file
(output/'science.json').write_text(json.dumps({'comparison':comparison,'full_b_V_sample_status_dof_saved_exact':True},indent=2)+'\n')
