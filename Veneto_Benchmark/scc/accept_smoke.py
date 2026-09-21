"""Accept scheduler, application, and independent-check evidence before production."""
import argparse
import json
import subprocess
import time
from pathlib import Path
from accounting import parse
from common import read_json, write_json


def accept(root, job_id):
    for attempt in range(10):
        query = subprocess.run(['qacct', '-j', str(job_id)], capture_output=True, text=True)
        accounting = parse(query.stdout, job_id, [1])
        if accounting['status'] != 'pending':
            break
        time.sleep(20)
    write_json(root/'results/smoke-accounting.json', accounting)
    assert accounting['status'] == 'pass', 'Successful smoke accounting required'
    task = root/'results/smoke/task-01'
    complete = read_json(task/'COMPLETE.json')
    assert complete['job_id'] == str(job_id) and complete['calls'] == 22
    checks = complete['oracle_checks']
    assert {(q['model'], q['threads']) for q in checks} == {(m,t) for m in ['ols','ppml'] for t in [1,28]}
    assert len(checks) == 4 and all(q['pass'] for q in checks)
    log = (root/'logs/smoke.log').read_text()
    assert 'VENETO_SCC_TASK_COMPLETE' in log and 'Traceback (most recent call last)' not in log
    rows = [read_json(p) for p in task.glob('*-r0-*.json')]
    assert len(rows) == 22
    assert len({(r['cell']['model'], r['arm'], r['cell']['threads']) for r in rows}) == 22
    assert all(r['measurement_fingerprint'] == complete['runtime_fingerprint'] for r in rows)
    assert all(r['rss_samples'] > 0 and r['failure_stage'] != 'before_target_invocation' for r in rows)
    authorization = read_json(root/'input/data-authorization.json')
    from common import sha256
    for name, info in authorization['files'].items():
        path = root/'input'/name
        assert path.stat().st_size == info['bytes'] and sha256(path) == info['sha256'], name
    result = {'status':'pass','job_id':str(job_id),'runtime_fingerprint':complete['runtime_fingerprint'],
              'calls':22,'independent_checks':checks,'accounting':accounting,
              'data_authorization_sha256':sha256(root/'input/data-authorization.json'),
              'comparator_outcomes':[{'model':r['cell']['model'],'arm':r['arm'],'threads':r['cell']['threads'],'status':r['status']} for r in rows]}
    write_json(root/'results/smoke-accepted.json',result)
    print('VENETO_SMOKE_ACCEPTED',flush=True)


if __name__ == '__main__':
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('job_id',type=int)
    args=parser.parse_args()
    try:
        accept(args.root,args.job_id)
    except Exception:
        import traceback
        traceback.print_exc()
        # SGE exit 100 keeps dependent jobs on hold when acceptance fails.
        raise SystemExit(100)
