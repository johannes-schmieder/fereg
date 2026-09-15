from pathlib import Path
import json,os,hashlib,statistics,sys,csv
from measure import measure
from common import sha256,write_json
root=Path(__file__).resolve().parents[1]
requests=json.loads((root/'requests.json').read_text());fixed=json.loads((root/'fixed-cases.json').read_text());bindings=json.loads((root/'input-bindings.json').read_text())
assert sha256(root/'fixed-cases.json')==bindings['fixed_manifest_sha256']
assert sha256(root/'requests.json')==bindings['requests_sha256']
for p,h in bindings['harness'].items():assert sha256(root/'harness'/p)==h
sources={'baseline':Path('${BASELINE_SOURCE}'),'candidate':root/'source'}
ident=json.loads((root/'identity.json').read_text())
for p,h in ident['files'].items():assert sha256(sources['candidate']/p)==h,p
artifacts={a:sha256(s/'dist/fereg_linux.plugin')for a,s in sources.items()}
assert artifacts['baseline']=='551e8759c5cf3e2558c94b6af1c139f2801971de8729f912afba2f8e41ed55ed'
assert 'MEMORY_SCREEN_LINUX_BUILD_PASS' in (root/'build.log').read_text()
(root/'work/refs').mkdir(parents=True,exist_ok=True)
manifest={'measurement':{'startup_timeout_seconds':90,'command_timeout_seconds':180,'rss_interval_seconds':0.01}}
def numeric_rows(path):
 with path.open()as f:return [r for r in csv.DictReader(f)]
def science(paths,model):
 prefix='feppml'if model=='ppml'else'fereg'
 names=['b','V']+[prefix+'_'+x for x in ['status','fit','dof','fe','cluster','subset','certificate']]
 if model=='ppml':names += [prefix+'_'+x for x in ['separation','inference','drop_counts','separation_sequence','receipt']]
 for name in names:
  a,b=[p/(name+'.csv')for p in paths.values()];assert a.exists()==b.exists(),name
  if a.exists():assert a.read_bytes()==b.read_bytes(),name
 a,b=[numeric_rows(p/(prefix+'_solver.csv'))for p in paths.values()]
 skip={14,15,16}if model=='ppml'else{9,10}
 assert [r for r in a if int(r['column'])not in skip]==[r for r in b if int(r['column'])not in skip],'solver non-time fields'
 for name in ['saved_outputs.csv']:
  a,b=[p/name for p in paths.values()]
  assert a.exists()==b.exists()
  if a.exists():assert a.read_bytes()==b.read_bytes(),name
 receipts=[json.loads((p/'result.json').read_text())for p in paths.values()]
 assert receipts[0]['sample_sha256']==receipts[1]['sample_sha256']
 assert receipts[0]['sample_rows']==receipts[1]['sample_rows']
 if model=='ppml':assert all(r['sample_rows']==65503 and r['metadata']['converged']==1 for r in receipts)
 return {'scientific_matrices_exact':names,'solver_nontime_exact':True,'sample_order_exact':True,'saved_outputs_exact':True}
rows=[]
for req in requests:
 data=root/'data'/req['data'];assert sha256(data)==req['data_sha256'];assert sha256(data.with_suffix('.json'))==req['metadata_sha256']
 for pair,order in enumerate(fixed['measurement']['arm_orders'],1):
  row={'case':req['id'],'pair':pair,'order':order};paths={}
  for arm in order:
   out=root/'runs'/f"{req['id']}-p{pair}-{arm}";paths[arm]=out
   receipt=measure(root,req['cell'],'feppml'if req['cell']['model']=='ppml'else'fereg','cold',data,data,out,manifest,source=sources[arm],solver=req['solver'],identity={'arm':arm,'source':fixed['baseline_source']if arm=='baseline'else ident['label'],'artifact_sha256':artifacts[arm],'reference_lock_sha256':ident['reference_lock_sha256'],'fixed_cases_sha256':bindings['fixed_manifest_sha256']})
   row[arm]=receipt;write_json(root/'partial-results.json',rows+[row]);print(req['id'],pair,arm,receipt['status'],receipt['seconds'],flush=True)
   if receipt['status']!='pass':raise RuntimeError('Preserved failed arm; no automatic retry')
  try:row['science']=science(paths,req['cell']['model'])
  except Exception as e:
   row['science_error']=repr(e);write_json(root/'partial-results.json',rows+[row]);raise
  rows.append(row);write_json(root/'partial-results.json',rows)
summary=[]
for req in requests:
 group=[r for r in rows if r['case']==req['id']];med={a:statistics.median(r[a]['seconds']for r in group)for a in ['baseline','candidate']};delta=med['candidate']-med['baseline'];ratio=med['candidate']/med['baseline']-1
 summary.append(dict(case=req['id'],median_seconds=med,delta_seconds=delta,relative_change=ratio,regression_requires_investigation=delta>0.01 and ratio>0.05))
write_json(root/'summary.json',dict(development_only=True,source=ident['label'],artifacts=artifacts,pairs=len(rows),summary=summary,all_science_exact=True,performance_gate_pass=not any(r['regression_requires_investigation']for r in summary)))
print('FIXED_MEMORY_SCREEN_COMPLETE',flush=True)
