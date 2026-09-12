import copy
import numpy as np
from common import sha256, write_json
from final_compare import baseline_compare, certify, matrix
from measure import measure
from preparation_compare import input_for
from run import BASE, MANIFEST

spec = dict(name="robust_missing", design="one_fe", n=983040, k=50, threads=16, vce="robust", missing=True)
seed=104729
data, warm = input_for(spec,seed), input_for(spec,seed+1,warm=True)
cell=dict(model="ols",section="width",effects=["g1"],**spec)
paths={}
for arm in ["baseline","candidate"]:
 source=BASE/arm
 path=BASE/"runs"/f"inference-pilot-{spec['name']}-s{seed}-{arm}"
 paths[arm]=path
 manifest=copy.deepcopy(MANIFEST)
 manifest["measurement"]["command_timeout_seconds"]=90
 result=measure(BASE,cell,"fereg","warm",data,warm,path,manifest,
  identity=dict(base_commit="f4cebe71bb8afcd29e5acb2e97a1d1875a340ba7",variant=arm,seed=seed,trace=True,plugin_sha256=sha256(source/"dist/fereg_macos.plugin")),source=source,trace=True)
 print(arm,result["status"],result["seconds"],flush=True)
 assert result["status"]=="pass"
 write_json(path/"native-science.json",certify(path))
 write_json(path/"phases.json",[l for l in (path/"stderr.txt").read_text().splitlines() if l.startswith("FEREG_PHASE|")])
science=baseline_compare(paths["candidate"],paths["baseline"])
for name in ["fereg_status","fereg_dof","fereg_fe","fereg_cluster","fereg_subset"]:
 a,b=(paths[arm]/(name+".csv") for arm in ["candidate","baseline"])
 assert a.exists()==b.exists()
 if a.exists(): assert a.read_bytes()==b.read_bytes(),name
cols=[0,1,2,3,10,11,12,13,14,15]
assert np.array_equal(matrix(paths["candidate"],"fereg_solver")[:,cols],matrix(paths["baseline"],"fereg_solver")[:,cols])
write_json(paths["candidate"]/"inference-science.json",science)
print(science,flush=True)
