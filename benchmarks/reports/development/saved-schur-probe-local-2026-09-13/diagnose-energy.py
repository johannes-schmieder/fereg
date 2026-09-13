from pathlib import Path
import copy, json
import measure as worker
from run import BASE, MANIFEST
from common import read_json

worker.HERE=BASE/'diagnostic-worker'
request=read_json(BASE/'runs/screen-pw_partial-s104729-baseline/request.json')
manifest=copy.deepcopy(MANIFEST)
manifest['measurement']['command_timeout_seconds']=180
result=worker.measure(BASE,request['cell'],'fereg','warm',Path(request['data']),
    Path(request['warm']),BASE/'runs/diagnostic-pw-energy-baseline',manifest,
    source=BASE/'energy-diagnostic-source',trace=True,identity={'purpose':'Noisy original failing worker replay; excluded from primary timing'})
print(json.dumps({'status':result['status'],'seconds':result['seconds'],'metadata':result['metadata']}),flush=True)
