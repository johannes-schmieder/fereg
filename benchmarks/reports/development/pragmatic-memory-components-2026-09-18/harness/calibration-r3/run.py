from pathlib import Path
import json,os,re,statistics,copy
import numpy as np
from measure import measure
from common import sha256,write_json
root=Path(__file__).resolve().parents[1];fixed=json.loads((root/'fixed-cases.json').read_text())
source=Path(os.environ['SOURCE_ROOT']);artifact=sha256(source/'dist/fereg_linux.plugin')
(root/'work/refs').mkdir(parents=True,exist_ok=True)
config={'measurement':{'startup_timeout_seconds':90,'command_timeout_seconds':180,'rss_interval_seconds':.01}}

def matrix(path):
 rows=np.genfromtxt(path,delimiter=',',names=True)
 return np.array(rows['value'],ndmin=1)

def compare(a,b,model):
 prefix='feppml' if model=='ppml' else 'fereg'
 exact=['status','dof','fe','cluster','subset','drop_counts','separation_sequence']
 numeric=['fit','certificate','inference','separation']
 for suffix in exact+numeric:
  left=a/(prefix+'_'+suffix+'.csv');right=b/(prefix+'_'+suffix+'.csv')
  assert left.exists()==right.exists(),suffix
  if not left.exists():continue
  l,r=matrix(left),matrix(right)
  assert l.shape==r.shape,suffix
  assert np.allclose(l,r,rtol=0 if suffix in exact else 1e-8,atol=0 if suffix in exact else 1e-9,equal_nan=True),suffix
 for name in ['b','V']:
  assert np.allclose(matrix(a/(name+'.csv')),matrix(b/(name+'.csv')),rtol=1e-8,atol=1e-10),name
 for name in ['saved_outputs.csv']:
  if (a/name).exists():
   l=np.genfromtxt(a/name,delimiter=',',skip_header=1);r=np.genfromtxt(b/name,delimiter=',',skip_header=1)
   assert l.shape==r.shape and np.allclose(l,r,rtol=1e-8,atol=1e-8,equal_nan=True),name

rows=[]
for req in fixed['cases']:
 # One synthetic calibration per case; performance comparisons below are fixed separately.
 cell=copy.deepcopy(req['cell']);cell['cmg_variant']='ordinary'
 out=root/'runs'/req['id']/'calibration';data=root/'data'/req['data']
 receipt=measure(root,cell,'feppml' if cell['model']=='ppml' else 'fereg','cold',data,data,out,config,source=source,solver=req['solver'],identity={'source_manifest':os.environ['SOURCE_MANIFEST_SHA256'],'artifact_sha256':artifact,'fixed_manifest_sha256':sha256(root/'fixed-cases.json')})
 row=dict(case=req['id'],receipt=receipt)
 text=(out/'stderr.txt').read_text();trace=re.findall(r'FEREG_WHOLE_ACTION_MEMORY_TERMINAL[^\n]+',text)
 row['native_terminal']=trace[-1] if trace else None
 rows.append(row);write_json(root/'calibration.json',rows)
 print(req['id'],receipt['status'],receipt['seconds'],row['native_terminal'],flush=True)
 if receipt['status']!='pass':raise RuntimeError('preserved failed calibration; no expansion')
 assert trace,'whole ABI terminal observation missing'
 assert (out/('feppml_memory.csv' if cell['model']=='ppml' else 'fereg_memory.csv')).exists()
print('PRAGMATIC_CALIBRATION_COMPLETE',flush=True)

pairs=[]
for req in fixed['cases']:
 if req['id'] not in fixed['pairs']['components']:continue
 for pair,order in enumerate(fixed['arm_orders'],1):
  row=dict(case=req['id'],pair=pair,arms={});paths={}
  for arm in order:
   cell=copy.deepcopy(req['cell']);cell['cmg_variant']=arm
   out=root/'runs'/req['id']/f'p{pair}-{arm}';paths[arm]=out;data=root/'data'/req['data']
   receipt=measure(root,cell,'feppml' if cell['model']=='ppml' else 'fereg','cold',data,data,out,config,source=source,solver=req['solver'],identity={'source_manifest':os.environ['SOURCE_MANIFEST_SHA256'],'artifact_sha256':artifact,'fixed_manifest_sha256':sha256(root/'fixed-cases.json'),'variant':arm})
   row['arms'][arm]=receipt;write_json(root/'pairs.json',pairs+[row]);print(req['id'],pair,arm,receipt['status'],receipt['seconds'],flush=True)
   if receipt['status']!='pass':raise RuntimeError('preserved failed pair; no expansion')
  assert row['arms']['ordinary']['sample_sha256']==row['arms']['components']['sample_sha256']
  compare(paths['ordinary'],paths['components'],req['cell']['model'])
  row['science']='pass';pairs.append(row);write_json(root/'pairs.json',pairs)
summary=[]
for case in fixed['pairs']['components']:
 med={a:statistics.median(r['arms'][a]['seconds'] for r in pairs if r['case']==case) for a in ['ordinary','components']}
 delta=med['components']-med['ordinary'];relative=delta/med['ordinary']
 summary.append(dict(case=case,medians=med,delta_seconds=delta,relative_change=relative,flag=delta>.01 and relative>.05))
write_json(root/'summary.json',dict(cases=summary,performance_pass=not any(r['flag'] for r in summary)))
print('PRAGMATIC_COMPONENT_PAIRS_COMPLETE',flush=True)
