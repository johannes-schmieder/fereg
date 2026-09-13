import os
from pathlib import Path
import json,copy,hashlib
import numpy as np
from measure import measure
from run import MANIFEST
from final_compare import certify,baseline_compare,matrix
root=Path(os.environ['EXPERIMENT_ROOT']);old=Path(os.environ['RECOVERY_ADMISSION_RUNS'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
ids=json.loads((root/'identities.json').read_text());ids['diagnostic_plugin']=sha(root/'diagnostic/dist/fereg_macos.plugin')
log=(root/'build.log').read_text();assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2 and 'actual-plugin ABI/self-test harness: PASS' in log
(root/'built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
results=[]
for name in ['unit_saved','pw_partial','low_fw']:
 ref=old/(name+'-candidate');req=json.loads((ref/'request.json').read_text());out=root/'runs'/name
 m=copy.deepcopy(MANIFEST);m['measurement']['command_timeout_seconds']=180
 r=measure(root,req['cell'],'fereg','warm',Path(req['data']),Path(req['warm']),out,m,source=root/'diagnostic',identity={'plugin':ids['diagnostic_plugin'],'purpose':'admission and scalar-plan diagnostic','protocol':sha(root/'protocol.md')})
 print(json.dumps({'name':name,'status':r['status'],'seconds_diagnostic':r['seconds']}),flush=True)
 assert r['status']=='pass',out
 (out/'native-science.json').write_text(json.dumps(certify(out),indent=2)+'\n')
 compare=baseline_compare(out,ref)
 for file in ['b','V','fereg_status','fereg_dof','fereg_fit','fereg_certificate','fereg_fe','fereg_cluster','fereg_subset','saved_outputs']:
  a,b=out/(file+'.csv'),ref/(file+'.csv');assert a.exists()==b.exists(),file
  if a.exists():assert a.read_bytes()==b.read_bytes(),(name,file)
 cols=[0,1,2,3,4,5,6,7,10,11,12,13,14,15]
 assert np.array_equal(matrix(out,'fereg_solver')[:,cols],matrix(ref,'fereg_solver')[:,cols]),name
 audit=[]
 for p in out.glob('*.log'):
  for line in p.read_text(errors='replace').splitlines():
   if line.startswith(('ADMISSION_AUDIT|','RECOVERY_AUDIT|','RECOVERY_PLAN_AUDIT|')):audit.append(line)
 row={'name':name,'seconds_diagnostic':r['seconds'],'science_exact':True,'solver_exact':True,'metadata':r['metadata'],'comparison':compare,'audit':audit}
 results.append(row);(root/'diagnostic-results.json').write_text(json.dumps(results,indent=2)+'\n');print('\n'.join(audit),flush=True)
