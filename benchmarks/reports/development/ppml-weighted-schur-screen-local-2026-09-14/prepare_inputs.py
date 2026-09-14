import os
from pathlib import Path
import json,hashlib
from ppml_preparation_compare import input_for
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
specs=[{'name':'dense_unit','design':'two_fe_dense','n':122880,'k':50,'threads':4,'vce':'robust'}, {'name':'strong_fw','design':'strong_d3','n':122880,'k':50,'threads':4,'weight':'[fw=w]','vce':'robust','fit_options':'exposure(exposure)','missing':True}, {'name':'dense_pw','design':'two_fe_dense','n':122880,'k':50,'threads':16,'weight':'[pw=w]','vce':'cluster c1 c2','fit_options':'offset(off)'}]
requests=[]
for spec in specs:
 data=input_for(spec,104729);warm=input_for(spec,104730,warm=True)
 requests.append({'name':spec['name'],'cell':dict(model='ppml',section='width',effects=['g1','g2'],**spec),'seed':104729,'warm_seed':104730,'data':str(data),'warm':str(warm),'data_sha256':sha(data),'warm_sha256':sha(warm),'data_metadata_sha256':sha(data.with_suffix('.json')),'warm_metadata_sha256':sha(warm.with_suffix('.json'))})
 print(spec['name'],sha(data),flush=True)
(r/'baseline-route-requests.json').write_text(json.dumps(requests,indent=2)+'\n')
print('SCHUR_INPUTS_TERMINAL_PASS',flush=True)
