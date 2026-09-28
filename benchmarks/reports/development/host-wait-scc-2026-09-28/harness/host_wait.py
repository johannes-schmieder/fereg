"""Same-artifact diagnostic: OpenMP waiting versus scoped Stata processors."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, sys

p=argparse.ArgumentParser()
for name in ('run-root','source','reference','scratch'):
    p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();root=a.run_root.resolve()
sys.path.insert(0,str(root/'systematic'))
import runner
from common import read_json,sha256,write_json
from generate import generate

assert os.environ['NSLOTS']=='28' and len(os.sched_getaffinity(0))==28
assert 'E5-2680 v4' in Path('/proc/cpuinfo').read_text()
plan=read_json(root/'registration.json');source=read_json(root/'source.json')
for name,digest in source['files'].items():
    assert sha256(a.source/name)==digest,name
assert sha256(a.source/'dist/fereg_linux.plugin')==plan['plugin_sha256']
manifest=read_json(a.reference/'input/manifest.json')
output=root/'results';output.mkdir(exist_ok=False);a.scratch.mkdir(exist_ok=False)
work=root/'worker-root';work.mkdir(exist_ok=False)
for name,target in [('candidate',a.source),('references',a.reference/'references'),('env',a.reference/'env')]:
    (work/name).symlink_to(target,target_is_directory=True)
original=(root/'systematic/stata_worker.do').read_text()
scoped=original.replace("    capture noisily `command'\n    local warm_rc = _rc", "    quietly set processors 1\n    capture noisily `command'\n    local warm_rc = _rc\n    quietly set processors `processors'\n    assert c(processors)==`processors'",1)
scoped=scoped.replace("timer on 91\ncapture noisily quietly `command'", "timer on 91\nquietly set processors 1\ncapture noisily quietly `command'",1)
scoped=scoped.replace("local returned_n = r(N)\ntimer off 91", "local returned_n = r(N)\nquietly set processors `processors'\nassert c(processors)==`processors'\ntimer off 91",1)
assert scoped.count('quietly set processors 1')==2
assert scoped.count("assert c(processors)==`processors'")==2
hostdir=root/'scoped-worker';hostdir.mkdir(exist_ok=False)
(hostdir/'stata_worker.do').write_text(scoped)
identities={'registration':sha256(root/'registration.json'),'source':sha256(root/'source.json'),'plugin':plan['plugin_sha256'],'harness':{str(f.relative_to(root)):sha256(f) for f in [root/'host_wait.py',*sorted((root/'systematic').glob('*')),hostdir/'stata_worker.do'] if f.is_file()}}
fingerprint=hashlib.sha256(json.dumps(identities,sort_keys=True).encode()).hexdigest()
write_json(output/'identity.json',identities)
records=[]
for case in plan['cases']:
    data=a.scratch/'input.dta';warm=a.scratch/'warm.dta';cell=case['cell']
    generated=generate(cell,case['seed'],data,manifest)
    if 'input_sha256' in case:assert generated['sha256']==case['input_sha256']
    generate(dict(cell,n=manifest['measurement']['warmup_rows'],section='main'),case['seed'],warm,manifest)
    for repetition in range(case['repetitions']):
        pair=output/case['id']/('pair-'+str(repetition+1));pair.mkdir(parents=True)
        order=plan['orders'][repetition%len(plan['orders'])]
        receipts={}
        for variant in order:
            for key in ('OMP_WAIT_POLICY','KMP_BLOCKTIME'):os.environ.pop(key,None)
            os.environ.update(plan['environment'].get(variant,{}))
            runner.HERE=hostdir if variant=='scoped_host1' else root/'systematic'
            arm='feppml' if cell['model']=='ppml' else 'fereg'
            r=runner.measure(work,cell,arm,'warm',data,warm,pair/variant,manifest,fingerprint,limits=plan['limits'])
            receipts[variant]=r
        comparisons={v:runner.compare(pair/v,pair/'default',manifest['science']) for v in ('passive','scoped_host1')}
        prefix='feppml' if cell['model']=='ppml' else 'fereg'
        for variant,c in comparisons.items():
            c['checks']['native_team']=all(r['metadata'].get(prefix+'_threads')==cell['threads'] and r['metadata'].get(prefix+'_requested_threads')==cell['threads'] for r in receipts.values())
            c['checks']['native_rank_dof']=all(receipts[variant]['metadata'].get(k)==receipts['default']['metadata'].get(k) for k in ('rank','df_a','df_r','df_resid'))
            if not all(c['checks'].values()):c['status']='accuracy_failure'
        record=dict(case=case['id'],repetition=repetition+1,seed=case['seed'],input_sha256=generated['sha256'],order=order,comparisons=comparisons)
        write_json(pair/'comparison.json',record);records.append(record);write_json(output/'progress.json',records)
        print(json.dumps(record),flush=True)
        if any(r['status']!='pass' for r in receipts.values()) or any(c['status']!='pass' for c in comparisons.values()):
            raise RuntimeError('diagnostic application/science failure; preserve without retries')
    for f in (data,warm,data.with_suffix('.json'),warm.with_suffix('.json')):f.unlink()
write_json(output/'complete.json',dict(complete=True,pairs=len(records),applications=3*len(records),diagnostic_only=True))
print('FEREG_HOST_WAIT_DIAGNOSTIC_COMPLETE',flush=True)
