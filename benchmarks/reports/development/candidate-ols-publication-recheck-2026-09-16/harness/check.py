from pathlib import Path
import csv, hashlib, json, os, platform, socket, statistics, subprocess
from measure import measure
from common import sha256, write_json
root=Path(__file__).resolve().parents[1]
plan=json.loads((root/'plan.json').read_text())
for name, expected in json.loads((root/'harness-hashes.json').read_text()).items():
    assert sha256(root/name)==expected,name
identity=json.loads((root/'identity.json').read_text())
artifacts=json.loads((root/'artifacts.json').read_text())
sources={'baseline':root/'baseline','candidate':root/'source'}
for arm, key in [('baseline','baseline_files'),('candidate','files')]:
    for name, expected in identity[key].items():assert sha256(sources[arm]/name)==expected,(arm,name)
    assert sha256(sources[arm]/'dist/fereg_linux.plugin')==artifacts[arm]
request=plan['request']; data=root/'data'/request['data']
assert sha256(data)==request['data_sha256']
assert sha256(data.with_suffix('.json'))==request['metadata_sha256']
(root/'work/refs').mkdir(parents=True,exist_ok=True)
write_json(root/'environment.json', {'job_id':os.environ.get('JOB_ID'),'host':socket.gethostname(),'platform':platform.platform(),
    'cpu_affinity':sorted(os.sched_getaffinity(0)),'lscpu':subprocess.check_output(['lscpu'],text=True),'artifacts':artifacts,
    'plan_sha256':sha256(root/'plan.json'),'source_snapshot':identity['label'],'baseline_source':identity['base_sha']})

def cells(path):
    with path.open() as stream:return list(csv.DictReader(stream))

def science(paths):
    required=['b','V','fereg_status','fereg_fit','fereg_dof','fereg_fe','fereg_certificate','saved_outputs']
    hashes={}
    for name in required+['fereg_cluster','fereg_subset']:
        left,right=[p/(name+'.csv') for p in paths]
        if name in required:assert left.is_file() and right.is_file(),name
        assert left.exists()==right.exists(),name
        if left.exists():
            a,b=sha256(left),sha256(right);assert a==b,name;hashes[name]=a
    left,right=[[row for row in cells(p/'fereg_solver.csv') if int(row['column']) not in [9,10]] for p in paths]
    assert left==right,'solver non-time fields'
    a,b=[json.loads((p/'result.json').read_text()) for p in paths]
    for key in ['sample_sha256','sample_rows','input_sha256']:assert a[key]==b[key],key
    return {'scientific_export_hashes':hashes,'solver_nontime_exact':True,'sample_order_exact':True,'input_exact':True}

rows=[]
for group in plan['groups']:
    row={'kind':group['kind'],'pair':group['pair'],'order':group['order']};paths={}
    for label in group['order']:
        arm=label if group['kind']=='comparison' else 'baseline'
        output=root/'runs'/f"{group['kind']}-p{group['pair']}-{label}";paths[label]=output
        receipt=measure(root,request['cell'],'fereg','cold',data,data,output,plan,source=sources[arm],solver=request['solver'],
            identity={'group':group['kind'],'label':label,'artifact_sha256':artifacts[arm],
                'source':identity['base_sha'] if arm=='baseline' else identity['label'],'plan_sha256':sha256(root/'plan.json'),
                'reference_lock_sha256':identity['reference_lock_sha256']})
        row[label]=receipt;write_json(root/'partial-results.json',rows+[row])
        print(group['kind'],group['pair'],label,receipt['status'],receipt['seconds'],flush=True)
        if receipt['status']!='pass':raise RuntimeError('Fit failure retained; no automatic retry')
        receipt['runtime']=cells(output/'fereg_runtime.csv');receipt['solver']=cells(output/'fereg_solver.csv')
        receipt['discarded_attempt_seconds']=None
        receipt['discarded_attempt_cost_coverage']='included in elapsed time; not separately observed'
    row['science']=science(list(paths.values()));rows.append(row);write_json(root/'partial-results.json',rows)

summary={}
for kind, names in [('control',['left','right']),('comparison',['baseline','candidate'])]:
    group=[r for r in rows if r['kind']==kind]
    medians={arm:statistics.median(r[arm]['seconds'] for r in group) for arm in names}
    delta=medians[names[1]]-medians[names[0]];ratio=medians[names[1]]/medians[names[0]]-1
    flag=abs(delta)>plan['review_threshold']['absolute_seconds'] and abs(ratio)>plan['review_threshold']['relative']
    summary[kind]={'medians':medians,'delta_seconds':delta,'relative_change':ratio,'absolute_shift_exceeds_threshold':flag,
      'ranges':{arm:[min(r[arm]['seconds'] for r in group),max(r[arm]['seconds'] for r in group)] for arm in names}}
summary['all_science_exact']=True;summary['original_failed_gate_preserved']=True
summary['outcome']='indeterminate_control_variation' if summary['control']['absolute_shift_exceeds_threshold'] else (
    'candidate_slowdown_reproduced' if summary['comparison']['absolute_shift_exceeds_threshold'] and summary['comparison']['delta_seconds']>0 else 'slowdown_not_reproduced_in_controlled_check')
write_json(root/'summary.json',summary)
print('OLS_PUBLICATION_CONTROL_CHECK_COMPLETE',flush=True)
