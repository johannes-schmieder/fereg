import os
from pathlib import Path
import json
import numpy as np
from ppml_one_fe_compare import numeric
from ppml_preparation_compare import named_runtime

def compare_routes(reference,other):
 paths=[reference,other];results=[json.loads((p/'result.json').read_text()) for p in paths]
 assert all(x['status']=='pass' for x in results)
 assert results[0]['sample_sha256']==results[1]['sample_sha256']
 differences={}
 def check(name,rows=None,exact=False,atol=1e-10,rtol=1e-7):
  a,b=[numeric(p,name) for p in paths];assert a.shape==b.shape,name
  if rows is not None:a,b=a[rows],b[rows]
  assert np.array_equal(np.isnan(a),np.isnan(b)),name
  finite=np.isfinite(a)&np.isfinite(b);assert np.all(finite|(np.isnan(a)&np.isnan(b))),name
  if exact:assert np.array_equal(a,b,equal_nan=True),name
  else:assert np.allclose(a,b,atol=atol,rtol=rtol,equal_nan=True),(name,np.nanmax(np.abs(a-b)))
  differences[name]=float(np.max(np.abs(a[finite]-b[finite]))) if finite.any() else 0.0
 for name in ['feppml_status','feppml_dof','feppml_fe','feppml_cluster','feppml_subset','feppml_separation','feppml_drop_counts','feppml_separation_sequence','feppml_receipt']:check(name,exact=True)
 check('b');check('V',rtol=1e-6)
 check('feppml_fit',rows=[*range(5),11,12,*range(16,27)],exact=True)
 check('feppml_fit',rows=list(range(5,11)),rtol=1e-7)
 check('feppml_inference',rows=[*range(8),10,14,15],exact=True)
 check('feppml_inference',rows=[8,9,11,12,13],rtol=1e-6)
 for p in paths:
  rt=named_runtime(p);assert numeric(p,'feppml_certificate')[-1,0]==1
  assert json.loads((p/'result.json').read_text())['metadata']['converged']==1
  assert rt['allocation_peak_delta']<=rt['modeled_selected_bytes'],p
 report={'reference':reference.name,'other':other.name,'sample_rank_status_dof_weights_cluster_separation_exact':True,'independent_runtime_certificates_pass':True,'registered_numeric_tolerances_pass':True,'modeled_native_peaks_admitted':True,'max_abs_differences':differences,'route_work_is_recorded_not_required_equal':True}
 (other/'route-science.json').write_text(json.dumps(report,indent=2)+'\n');return report
