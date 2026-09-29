"""Collect exact terminal diagnostic evidence, including refused/failed candidates."""
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import subprocess
import sys

BASE=Path('/projectnb/welfgr/fereg/dev')
STAGES={'cg':('nonretaining-cg','7778963',8),'schwarz':('schwarz-imbalance','7778964',8),
        'comparator':('comparator-state','7778965',4),'public':('solver-public','7778967',16)}

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text())

def collect(stage):
    name,jid,slots=STAGES[stage];root=BASE/('20260929-'+name+'-02')
    sys.path.insert(0,str(BASE/'20260929-solver-public-02'))
    from accounting import parse
    for path,digest in read(root/'deployment.json').items():assert sha(root/path)==digest,path
    raw=subprocess.run(['qacct','-j',jid],capture_output=True,text=True,check=True,timeout=120)
    accounting=parse(raw.stdout,jid,[1],slots);assert accounting['status']!='pending'
    logs=list((root/'logs').glob('*.o'+jid));assert len(logs)==1
    log=logs[0].read_text();files={}
    def bind(path):
        relative=str(path.relative_to(root));value=read(path)
        files[relative]=dict(sha256=sha(path),data=value);return value
    bind(root/'registration.json');bind(root/'deployment.json');bind(root/'submission.json')
    summary=bind(root/('results/complete.json' if stage=='comparator' else 'output/summary.json'))
    if stage=='comparator':
        comparison=bind(root/'results/comparison.json');bind(root/'results/oracle-tests.json')
        fixture=bind(root/'results/gtools/receipt.json')
        checks=root/'results/gtools/checks.tsv';files['results/gtools/checks.tsv']=dict(sha256=sha(checks),text=checks.read_text())
        plan=read(root/'registration.json')
        assert set(comparison)=={c['id'] for c in plan['cases']}
        assert all(set(c['profiles'])==set(plan['profiles']) for c in comparison.values())
        assert all(v['application']=='pass' for c in comparison.values() for v in c['profiles'].values())
        assert summary['diagnostic_complete'] and summary['gtools_fixture_passed'] and fixture['passed']
        assert log.splitlines().count('COMPARATOR_INDEPENDENT_ORACLES_PASS')==1
        assert log.splitlines().count('COMPARATOR_STATE_DIAGNOSTIC_COMPLETE')==1
        assert log.splitlines().count('FEREG_COMPARATOR_STATE_SUCCESS job='+jid)==1
        assert accounting['status']=='pass'
    else:
        for p in sorted((root/'output').glob('*source.json')):bind(p)
        if stage=='cg':
            from run import core_passed
            arm=next(a for a in read(BASE/'20260929-solver-public-02/registration.json')['arms'] if a['name']==name)
            assert core_passed(arm,summary,accounting,log)
        if stage=='schwarz':
            assert log.splitlines().count('FEREG_SCHWARZ_CANDIDATE_CORE_COMPLETE')==1
            assert len(summary['tests'])==7 and summary['distinct_tests']==120
            for i,t in enumerate(summary['tests'],1):
                p=root/'output'/('test-'+str(i)+'.txt');assert sha(p)==t['log_sha256']
                if not t['passed']:files[str(p.relative_to(root))]=dict(sha256=sha(p),text=p.read_text())
        if stage=='public':
            from run import fixture_passed
            for arm,record in summary['arms'].items():
                for i,test in enumerate(record['tests']):
                    p=root/'output'/arm/('fixture-'+str(i))/'summary.json'
                    if p.exists():
                        data=bind(p);assert test['passed']==(test['returncode']==0 and test['status']=='finished' and fixture_passed(data,[test['fixture']]))
                for p in (root/'output').glob(arm+'-*.json'):
                    if str(p.relative_to(root)) not in files:bind(p)
            p=root/'output/baseline-probe/summary.json'
            if p.exists():bind(p)
            assert log.splitlines().count('FEREG_SOLVER_PUBLIC_COMPLETE')==1
    value=dict(job_id=jid,accounting=accounting,summary=summary,files=files,
               terminal_log_sha256=sha(logs[0]),terminal_markers=[x for x in log.splitlines() if x.startswith('FEREG_')],
               production_adopted=False)
    destination=BASE/'20260929-harness-repair-01'/(stage+'-results.json')
    assert not destination.exists()
    destination.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
    print(stage,accounting['status'],flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('stages',nargs='+',choices=list(STAGES));args=parser.parse_args()
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(args.stages)) as pool:list(pool.map(collect,args.stages))
