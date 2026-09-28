from pathlib import Path
import sys,json,csv,math
r=Path('/projectnb/welfgr/fereg/runs/20260928-current-sweep-161b9491');sys.path.insert(0,str(r/'controller'))
from common import read_json,sha256,write_json
from provenance import verify
identity=verify(r);tol=read_json(r/'input/manifest.json')['science'];records=[]
for p in sorted((r/'smoke-results').glob('*/warm-1')):
 g=read_json(p/'GROUP.COMPLETE.json');o=read_json(p/'oracle.json');assert o['status']=='pass'
 for arm in g['arm_status']:
  d=read_json(p/arm/'result.json');names=o['coefficient_names']
  with (p/arm/'coefficients.csv').open() as stream:b={v['name']:(float(v['value']),float(v['se'])) for v in csv.DictReader(stream)}
  with (p/arm/'covariance.csv').open() as stream:V={(v['row'],v['column']):float(v['value']) for v in csv.DictReader(stream)}
  checks=dict(sample=d['sample_sha256']==o['sample_sha256'],converged=d['metadata']['converged']==1)
  errors={}
  for kind,actual,expected,rt,at in [
   ('coefficients',[b[n][0] for n in names],o['coefficients'],'coefficient_rtol','coefficient_atol'),
   ('standard_errors',[b[n][1] for n in names],[math.sqrt(o['covariance'][i][i]) for i in range(len(names))],'vce_rtol','vce_atol'),
   ('covariance',[V[a,b] for a in names for b in names],[v for row in o['covariance'] for v in row],'vce_rtol','vce_atol')]:
   normalized=[abs(a-e)/(tol[at]+tol[rt]*abs(e)) for a,e in zip(actual,expected)]
   checks[kind]=all(math.isfinite(v) and v<=1 for v in normalized);errors[kind]=max(normalized)
  objective='rss' if g['cell']['model']=='ols' else 'deviance'
  observed=d['metadata'][objective];expected=o[objective]
  # Same objective criterion as the governing smoke, including math.isclose's symmetric scale.
  checks['objective']=math.isclose(observed,expected,rel_tol=tol['objective_rtol'],abs_tol=tol['objective_atol'])
  records.append(dict(cell=g['cell'],arm=arm,checks=checks,maximum_error_in_envelope_units=errors,
   oracle_sha256=sha256(p/'oracle.json'),input_sha256=read_json(p/'input.json')['sha256'],
   result_sha256=sha256(p/arm/'result.json'),coefficients_sha256=sha256(p/arm/'coefficients.csv'),covariance_sha256=sha256(p/arm/'covariance.csv')))
output=dict(fingerprint=identity,authority='Existing independent smoke explicit-design oracles; no new estimator fit or timing eligibility changes.',records=records)
write_json(r/'env/smoke-reference-oracles.json',output);print(json.dumps(output))
