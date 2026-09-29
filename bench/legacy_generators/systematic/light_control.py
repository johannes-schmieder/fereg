"""Bounded smoke/pilot gates for development; no release qualification."""
import argparse
from pathlib import Path

from common import cell_id, manifest_for, read_json, sha256, write_json
from light import active_arms, skips
from provenance import verify


def verify_acceptance(root, mode):
    receipt = read_json(root / f'env/{mode.upper()}.ACCEPTED.json')
    if receipt['measurement_fingerprint'] != verify(root) or receipt['mode'] != mode:
        raise ValueError('Acceptance belongs to another runtime or stage')
    if sha256(root / receipt['plan']) != receipt['plan_sha256']:
        raise ValueError('Accepted plan changed')
    for name, digest in receipt['evidence'].items():
        if sha256(root / name) != digest:
            raise ValueError(f'Accepted evidence changed: {name}')
    return receipt


def accept(root, plan_path):
    plan = read_json(plan_path)
    mode = plan['mode']
    if mode not in ('smoke', 'pilot'):
        raise ValueError('Only smoke and pilot have stage gates')
    target = root / f'env/{mode.upper()}.ACCEPTED.json'
    if target.exists():
        return verify_acceptance(root, mode)
    identity = verify(root)
    manifest = manifest_for(root)
    if plan['manifest_sha256'] != sha256(root/'input/manifest.json'):
        raise ValueError('Plan belongs to another manifest')
    evidence, failures, diagnostics = {}, [], []
    def bind(path):
        evidence[str(path.relative_to(root))] = sha256(path)
        return read_json(path)
    accounting = {}
    for path in (root/'accounting').glob('*.json'):
        receipt = bind(path)
        for task in receipt['records']:
            accounting[str(receipt['job_id']),task['task_id']] = task
    seen = set()
    reached = set()
    for i, bundle in enumerate(plan['bundles'], 1):
        result = bind(root/'bundle-results'/f'{plan_path.stem}-{i}.json')
        if (result['status'] != 'complete' or result['pending'] or
            result['plan_sha256'] != sha256(plan_path) or result['measurement_fingerprint'] != identity or
            result['completed'] != bundle['groups']):
            raise ValueError('Bundle is not complete under the registered plan')
        for group in bundle['groups']:
            cell, rep = group['cell'], group['repetition']
            key = (cell_id(cell), rep)
            if key in seen:raise ValueError('Duplicate planned comparison')
            seen.add(key)
            directory = root/plan['results_directory']/cell_id(cell)/f'{"cold" if rep==0 else "warm"}-{rep}'
            record = bind(directory/'GROUP.COMPLETE.json')
            if (record['measurement_fingerprint'] != identity or record['cell'] != cell or record['repetition'] != rep or
                set(record['arm_status']) != set(active_arms(group,manifest)) or record['skips'] != skips(cell,rep,manifest)):
                raise ValueError('Group identity or registered arms changed')
            candidate = manifest['arms'][cell['model']][0]
            if record['arm_status'][candidate] != 'pass':failures.append([key,'native command failed'])
            if mode == 'smoke' and (not record['oracle_checks'] or not all(record['oracle_checks'].values())):
                failures.append([key,'independent oracle failed'])
            for arm in record['arm_status']:
                arm_record = bind(directory/arm/'result.json')
                task = arm_record['task_id']
                task = int(task) if str(task).isdigit() else 1
                scheduler = accounting.get((str(arm_record['job_id']),task))
                if not scheduler or any(scheduler[k] != v for k,v in [('failed',0),('exit_status',0),('slots',28)]):
                    raise ValueError('Missing or failed scheduler accounting')
                if arm_record['measurement_fingerprint'] != identity or arm_record['status'] != record['arm_status'][arm]:
                    raise ValueError('Inconsistent arm receipt')
                if arm_record['status'] == 'pass':reached.add(arm)
                if arm_record['status'] != 'pass':
                    diagnostics.append([key,arm,arm_record['status']])
            for comparison in record['comparisons']:
                if comparison['status'] != 'pass':diagnostics.append([key,comparison['right'],comparison['status']])
            if mode == 'pilot' and not any(c['status']=='pass' and c['checks'].get('sample_exact') for c in record['comparisons']):
                failures.append([key,'no scientifically comparable pilot reference'])
    if mode == 'smoke':
        expected = {a for arms in manifest['arms'].values() for a in arms}
        if expected-reached:failures.append(['comparator launch coverage',sorted(expected-reached)])
    if failures:
        write_json(root/f'env/{mode.upper()}.REJECTED.json', dict(failures=failures,diagnostics=diagnostics))
        raise ValueError(f'{mode} rejected: {len(failures)} native/oracle/pair failures; inspect rejection receipt')
    receipt = dict(mode=mode,measurement_fingerprint=identity,plan=str(plan_path.relative_to(root)),
                   plan_sha256=sha256(plan_path),groups=len(seen),evidence=evidence,diagnostics=diagnostics)
    write_json(target,receipt)
    return receipt


if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('run_root',type=Path)
    parser.add_argument('plan',type=Path)
    args=parser.parse_args()
    result=accept(args.run_root,args.plan)
    print(dict(mode=result['mode'],groups=result['groups'],diagnostics=len(result['diagnostics']),status='accepted'))
