#!/usr/bin/env python3
"""Bounded, serial, prepared-Schur discovery on a verified isolated SCC node."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import statistics
import subprocess
import threading
import time
import xml.etree.ElementTree as ET

ARMS = ['diagonal', 'eliminate', 'forest', 'shallow', 'full',
        'diagonal-full', 'forest-full', 'shallow-full']
CASES = ['expander', 'fringe', 'clusters', 'grid', 'hubs']
SEED = 20261016
PROTECTED_ARMS = ARMS + ['diagonal-q14', 'diagonal-q1', 'forest-q14', 'forest-q1', 'full-controlled']
ORIGINAL_CELLS = [(c, 4096, rhs, kind) for c in CASES
                  for rhs, kind in [(1, 'cut'), (1, 'random'), (4, 'mixed')]]
PROTECTED_CELLS = ORIGINAL_CELLS + [
    ('grid', 8192, 4, 'mixed'), ('clusters', 8192, 4, 'mixed'),
    ('hubs', 8192, 1, 'random'), ('mosaic', 8192, 1, 'cut'),
    ('mosaic', 8192, 4, 'mixed'), ('expander', 4096, 4, 'rev'),
    ('fringe', 4096, 4, 'rev')]

SUPPLEMENT_CELLS = [('grid', 8100, 4, 'mixed')]
PERSISTENT_BASE_ARMS = ['diagonal','forest','full','diagonal-full','forest-full','full-controlled','full-chunked'] + [f'{starter}-{mode}{share}' for mode in ['q','p','c'] for starter in ['diagonal','forest'] for share in ['14','1']]
PREVIOUS_ARMS = ['previous-'+a for a in ['full','full-controlled','diagonal-q14','diagonal-q1','forest-q14','forest-q1']]
PERSISTENT_ARMS = PERSISTENT_BASE_ARMS + PREVIOUS_ARMS
PERSISTENT_CELLS = [('expander',4096,4,'mixed'),('expander',4096,4,'rev'),
    ('fringe',4096,4,'mixed'),('clusters',4096,1,'cut'),('grid',4096,4,'mixed'),
    ('grid',8100,4,'mixed'),('mosaic',8192,4,'mixed'),('hubs',8192,1,'random')]



ROUTER_ARMS = ['diagonal','forest','full','diagonal-full','auto-commit']
ROUTER_CELLS = PERSISTENT_CELLS + [
    ('sparse-expander',4096,4,'mixed'),('sparse-expander',4096,4,'rev'),
    ('weighted-expander',4096,4,'mixed'),('weighted-expander',4096,4,'rev'),
    ('mosaic',8192,4,'components'),('mosaic',8192,4,'components-rev')]

ADMITTED_ARMS = ['diagonal','forest','full','diagonal-full','auto-admitted']
FORECAST_ARMS = ['diagonal','forest','full','auto-admitted','auto-forecast']
CONFIRMATION_CELLS = [(case,6144,4,kind) for case in ['expander','sparse-expander','weighted-expander'] for kind in ['mixed','rev']] + [
    ('fringe',6144,4,'rev'),('clusters',6144,1,'cut'),('grid',6400,4,'mixed'),
    ('grid',10000,4,'rev'),('hubs',12288,1,'random'),('mosaic',12800,4,'mixed'),
    ('mosaic',12800,4,'components'),('mosaic',12800,4,'components-rev')]
ROUTING_PROTOCOLS = ['router','admitted-router','forecast-discovery','forecast-confirmation']
ADMITTED_GATE_ARMS = ['full','full-admitted']

GUARDED_ARMS = ['full','full-guarded']
GUARDED_CELLS = [('expander',4096,4,'mixed'),('fringe',4096,4,'mixed'),
    ('clusters',4096,1,'cut'),('grid',8100,4,'mixed'),('hubs',8192,1,'random')]


def protocol_config(protocol):
    if protocol == 'forecast-discovery':
        return FORECAST_ARMS, ROUTER_CELLS, 20261024, 1800
    if protocol == 'forecast-confirmation':
        return FORECAST_ARMS, CONFIRMATION_CELLS, 20261025, 1800
    if protocol == 'admitted-router':
        return ADMITTED_ARMS, ROUTER_CELLS, 20261022, 1800
    if protocol == 'admitted-gate':
        return ADMITTED_GATE_ARMS, GUARDED_CELLS, 20261023, 300
    if protocol == 'router':
        return ROUTER_ARMS, ROUTER_CELLS, 20261021, 1800
    if protocol == 'guarded-gate':
        return GUARDED_ARMS, GUARDED_CELLS, 20261020, 300
    if protocol == 'persistent':
        return PERSISTENT_ARMS, PERSISTENT_CELLS, 20261019, 1800
    if protocol == 'protected-grid':
        return PROTECTED_ARMS, SUPPLEMENT_CELLS, 20261018, 600
    if protocol == 'protected':
        return PROTECTED_ARMS, PROTECTED_CELLS, 20261017, 1800
    return ARMS, ORIGINAL_CELLS, SEED, 1800


def validate_manifest(cells, arms):
    assert len(cells) == len(set(cells)) and len(arms) == len(set(arms))
    assert all(a in PROTECTED_ARMS + PERSISTENT_ARMS + GUARDED_ARMS + ROUTER_ARMS + ADMITTED_ARMS + ADMITTED_GATE_ARMS + FORECAST_ARMS for a in arms)
    for case,n,rhs,kind in cells:
        assert case in CASES + ['mosaic','sparse-expander','weighted-expander'] and n >= 64 and rhs > 0
        assert kind in ['cut','random','mixed','rev','components','components-rev']
        if case == 'grid':
            assert math.isqrt(n)**2 == n, ('grid requires a square vertex count', n)
        if case == 'mosaic':
            assert n >= 128 and n % 2 == 0 and math.isqrt(n//2)**2 == n//2, ('mosaic grid half must be square',n)


def preflight(binary, output, cells, arms, cpu, check=lambda: None):
    path = output / 'preflight.jsonl'
    assert not path.exists(), 'never overwrite preflight evidence'
    for case,n,rhs,kind in cells:
        for arm in arms:
            check()
            result = invoke(binary,case,n,rhs,arm,cpu,kind if kind in ['random','rev','components','components-rev'] else 'cut')
            append(path,dict(case=case,n=n,rhs_count=rhs,rhs_kind=kind,arm=arm,result=result))
            check()
            # Structured numerical/resource failures and timeouts are retained outcomes.
            # Malformed output, panic, wrong identity or an invalid success blocks timing.
            assert not (result['status']=='failed' and 'stdout' in result), result


def signature(result):
    if result['status'] != 'ok':
        return result['status'], result.get('reason')
    decisions = [{k: v for k, v in d.items() if not k.endswith('_seconds')}
                 for d in result.get('decisions', [])]
    return tuple(result[k] for k in ['digest', 'iterations', 'upgrades', 'certificates',
                 'relative_residual', 'graph_builds', 'coarse_n', 'tail_levels']) + (decisions,)



def command(argv):
    result = subprocess.run(argv, capture_output=True, text=True, timeout=15, check=True)
    return result.stdout


def append(path, row):
    with path.open('a') as f:
        f.write(json.dumps(row, sort_keys=True, allow_nan=False) + '\n')


def scheduler_snapshot():
    host = command(['hostname', '-s']).strip()
    raw = command(['qstat', '-xml', '-u', '*'])
    jobs = []
    for job in ET.fromstring(raw).iter('job_list'):
        queue = job.findtext('queue_name', '')
        if queue.split('@')[-1].split('.')[0] == host:
            jobs.append({'id': job.findtext('JB_job_number'), 'queue': queue,
                         'slots': job.findtext('slots')})
    if not jobs or any(j['id'] != os.environ['JOB_ID'] for j in jobs):
        raise RuntimeError('node has missing/competing scheduler allocation: ' + repr(jobs))
    return {'host': host, 'jobs': jobs, 'raw': raw}


def configuration():
    assert os.environ.get('NSLOTS') == '28'
    assert os.environ.get('PE') == 'omp28'
    host = command(['hostname', '-s']).strip()
    pe = command(['qconf', '-sp', 'omp28'])
    assert 'allocation_rule' in pe and '$pe_slots' in pe
    quota = command(['qconf', '-srqs', 'max_slots_per_host'])
    assert 'enabled' in quota and 'TRUE' in quota
    assert 'limit hosts {*} to slots=$num_proc' in ' '.join(quota.split())
    host_config = command(['qconf', '-se', host])
    assert re.search(r'(?:^|[,\s])num_proc=28(?:[,\s]|$)', host_config)
    assert re.search(r'^processors\s+28\s*$', host_config, re.MULTILINE)
    complexes = command(['qconf', '-sc'])
    fields = {r.split()[0]: r.split() for r in complexes.splitlines() if r.strip() and not r.lstrip().startswith('#')}
    assert fields['num_proc'][2:6] == ['INT', '==', 'YES', 'NO']
    assert fields['slots'][2:6] == ['INT', '<=', 'YES', 'YES']
    topology = command(['lscpu', '-p=CPU,CORE,SOCKET,ONLINE'])
    physical = [line.split(',') for line in topology.splitlines()
                if line and not line.startswith('#') and line.endswith(',Y')]
    affinity = sorted(os.sched_getaffinity(0))
    assert len(physical) == len({tuple(r[1:3]) for r in physical}) == 28
    assert sorted(int(r[0]) for r in physical) == affinity
    hostfile = Path(os.environ['PE_HOSTFILE']).read_text()
    allocations = [line.split() for line in hostfile.splitlines() if line.strip()]
    assert len(allocations) == 1 and allocations[0][1] == '28'
    assert allocations[0][0].split('.')[0] == host
    return {'job': os.environ['JOB_ID'], 'host': host, 'pe': pe, 'rqs': quota,
            'host_configuration_raw': host_config, 'complexes_raw': complexes, 'topology': topology,
            'affinity': affinity, 'pe_hostfile': hostfile, 'compute_threads': 1}


def cpu_seconds(value):
    days, sep, value = value.rpartition('-')
    parts = [int(p) for p in value.split(':')]
    total = 0
    for p in parts:
        total = total * 60 + p
    return total + (int(days) * 86400 if sep else 0)


class Observer:
    def __init__(self, path):
        self.path = path
        self.stop = threading.Event()
        self.failure = None
        self.previous = {}
        self.thread = threading.Thread(target=self.loop, daemon=True)

    def sample(self):
        raw = command(['ps', '-eo', 'pid=,ppid=,time=,comm='])
        rows = {}
        for line in raw.splitlines():
            pid, ppid, elapsed, name = line.strip().split(None, 3)
            rows[int(pid)] = (int(ppid), cpu_seconds(elapsed), Path(name.strip()).name)
        own = {os.getpid()}
        while True:
            children = {pid for pid, row in rows.items() if row[0] in own}
            expanded = own | children
            if expanded == own:
                break
            own = expanded
        competitors = []
        names = ('cargo', 'rustc', 'clippy', 'python', 'stata', 'fereg',
                 'staged-cmg', 'julia', 'matlab', 'Rscript')
        for pid, (_, seconds, name) in rows.items():
            previous = self.previous.get(pid)
            if (pid not in own and previous and previous[2] == name
                    and seconds - previous[1] >= 1 and name.startswith(names)):
                competitors.append({'pid': pid, 'name': name, 'cpu_seconds': seconds - previous[1]})
        self.previous = rows
        evidence = scheduler_snapshot()
        append(self.path, {'time': time.time(), 'scheduler': evidence, 'processes': raw,
                           'competing_compute': competitors})
        if competitors:
            raise RuntimeError('competing compute observed')

    def loop(self):
        try:
            while not self.stop.wait(10):
                self.sample()
        except Exception as error:
            self.failure = str(error)

    def check(self):
        if self.failure:
            raise RuntimeError(self.failure)


def invoke(binary, case, n, rhs, arm, cpu=None, pattern='cut'):
    if arm.startswith('previous-'):
        binary = Path(os.environ['STAGED_PREVIOUS_BINARY'])
        arm = arm.removeprefix('previous-')
    argv = [str(binary), case, str(n), str(rhs), arm, pattern]
    if cpu is not None:
        argv = ['taskset', '-c', str(cpu)] + argv
    try:
        invoked=time.monotonic()
        run = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        result = json.loads(run.stdout)
        result['process_seconds']=time.monotonic()-invoked
        if run.returncode != 0:
            result['status'] = 'failed'
            result['exit_status'] = run.returncode
        if result['status'] == 'ok':
            assert (result['case'], result['n'], result['rhs'], result['arm']) == (case, n, rhs, arm)
            assert math.isfinite(result['seconds']) and result['seconds'] > 0
            assert 0 <= result['relative_residual'] <= 1e-8
            assert result['graph_builds'] <= (rhs if '-q' in arm else 1)
            for d in result.get('decisions', []):
                if 'optional_work' in d:
                    assert d['optional_work'] <= d['incumbent_work'] // d['share_denominator']
                    assert d['optional_work'] == sum(d[k] for k in ['build_work', 'challenger_work', 'control_work'])
                    assert d['incumbent_iterations'] + d['challenger_iterations'] <= 16000
                    assert d['memory_bound'] <= 1 << 30
        return result
    except subprocess.TimeoutExpired:
        return {'status': 'timeout','process_seconds':time.monotonic()-invoked}
    except (ValueError, AssertionError, KeyError, TypeError) as error:
        return {'status': 'failed', 'reason': str(error), 'stdout': run.stdout, 'stderr': run.stderr}


def summarize(path, output, protocol='original'):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    arms, cells, seed, _ = protocol_config(protocol)
    results = []
    for case, n, rhs, kind in cells:
        cell = [r for r in rows if (r['case'], r.get('n',4096), r['rhs_count'], r['rhs_kind']) == (case,n,rhs,kind) and r['phase'] == 'measure']
        for arm in arms:
            arm_rows = [r for r in cell if r['arm'] == arm]
            paired = {}
            for comparator in ['diagonal', 'forest', 'full', 'diagonal-full', 'forest-full'] + ([a for a in PERSISTENT_ARMS if a not in ['diagonal','forest','full','diagonal-full','forest-full']] if protocol=='persistent' else []):
                ratios = []
                for block in range(6):
                    a = [r['result'] for r in arm_rows if r['block'] == block]
                    b = [r['result'] for r in cell if r['arm'] == comparator and r['block'] == block]
                    if len(a) == len(b) == 2 and all(r['status'] == 'ok' for r in a+b):
                        ratios.append(math.exp(sum(math.log(r['seconds']) for r in a)/2 - sum(math.log(r['seconds']) for r in b)/2))
                paired[comparator] = ratios
            ratios = paired['diagonal']
            success = [r['result'] for r in arm_rows if r['result']['status'] == 'ok']
            results.append({'case':case, 'n':n, 'rhs':rhs, 'rhs_kind':kind, 'arm':arm,
                'observations':len(arm_rows), 'failures':len(arm_rows)-len(success),
                'block_ratios_to_diagonal':ratios, 'paired_block_ratios':paired,
                'median_ratio':statistics.median(ratios) if len(ratios)==6 else None,
                'mean_seconds':statistics.mean(r['seconds'] for r in success) if success else None,
                'mean_setup_seconds':statistics.mean(r['setup_seconds'] for r in success) if success else None,
                'iterations':sorted({r['iterations'] for r in success}),
                'upgrades':sorted({r['upgrades'] for r in success}),
                'digests':sorted({r['digest'] for r in success}),
                'decisions':[{k:v for k,v in d.items() if not k.endswith('_seconds')}
                             for d in success[0].get('decisions',[])] if success else []})
    output.write_text(json.dumps({'scope':'serial prepared-Schur SCC discovery only',
        'protocol':protocol, 'blocks':6, 'seed':seed,
        'results':results},indent=2)+'\n')


def legacy_replay(binary, output):
    reference = json.loads(Path(__file__).with_name('discovery.json').read_text())['discovery']['results']
    for case, n, rhs, kind in ORIGINAL_CELLS:
        for arm in ARMS + ['full-controlled']:
            baseline = next(r for r in reference if (r['case'],r['rhs'],r['rhs_kind'],r['arm']) ==
                            (case,rhs,kind,'full' if arm=='full-controlled' else arm))
            result = invoke(binary,case,n,rhs,arm,pattern='random' if kind=='random' else 'cut')
            append(output / 'legacy-replay.jsonl',dict(case=case,n=n,rhs=rhs,kind=kind,arm=arm,result=result))
            assert result['status']=='ok', result
            assert result['digest'] in baseline['digests'], (case,kind,arm,'digest')
            assert result['iterations'] in baseline['iterations'], (case,kind,arm,'iterations')
            if arm!='full-controlled':
                assert result['upgrades'] in baseline['upgrades']


def protected_replay(binary,output):
    reference=json.loads(Path(__file__).with_name('protected-discovery.json').read_text())['campaigns'][0]['summary']['results']
    count=0
    for prior in reference:
        if prior['arm'] not in ['diagonal-q14','diagonal-q1','forest-q14','forest-q1'] or prior['failures']:
            continue
        result=invoke(binary,prior['case'],prior['n'],prior['rhs'],prior['arm'],pattern=prior['rhs_kind'] if prior['rhs_kind'] in ['random','rev'] else 'cut')
        append(output/'protected-replay.jsonl',dict(prior=prior['arm'],case=prior['case'],n=prior['n'],kind=prior['rhs_kind'],result=result))
        assert result['status']=='ok',result
        assert result['digest'] in prior['digests'] and result['iterations'] in prior['iterations']
        assert result['upgrades'] in prior['upgrades']
        decisions=[{k:v for k,v in d.items() if not k.endswith('_seconds')} for d in result['decisions']]
        assert decisions==prior['decisions'],(prior['case'],prior['rhs_kind'],prior['arm'],'control replay')
        count+=1
    assert count==80,count


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['smoke', 'measure', 'summary', 'pilot'])
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--protocol', choices=['original','protected','protected-grid','persistent','guarded-gate','router','admitted-gate','admitted-router','forecast-discovery','forecast-confirmation'], default='original')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--budget-seconds',type=int)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    arms, manifest, seed, budget = protocol_config(args.protocol)
    if args.budget_seconds is not None:
        assert args.protocol in ROUTING_PROTOCOLS and 60<=args.budget_seconds<=1800
        budget=args.budget_seconds
    rows = args.output / 'observations.jsonl'
    if args.mode == 'summary':
        summarize(rows, args.output / 'summary.json', args.protocol)
        return
    validate_manifest(manifest, arms)
    assert args.binary and args.binary.is_file()
    if args.mode == 'smoke':
        for case in CASES:
            for arm in arms:
                result = invoke(args.binary, case, 256, 2, arm)
                append(args.output / 'smoke.jsonl', {'case': case, 'arm': arm, 'result': result})
                assert result['status'] == 'ok', (case, arm, result)
        if args.protocol in ['protected','persistent']:
            legacy_replay(args.binary, args.output)
        if args.protocol == 'persistent':
            protected_replay(args.binary,args.output)
            for arm in ['diagonal-c1','forest-c1']:
                result=invoke(args.binary,'clusters',8192,4,arm)
                append(args.output/'cluster-diagnostic.jsonl',dict(case='clusters',n=8192,rhs_count=4,arm=arm,result=result))
                assert not (result['status']=='failed' and 'stdout' in result),result
        if args.protocol in ROUTING_PROTOCOLS:
            for arm in arms:
                result=invoke(args.binary,'clusters',8192,1,arm)
                append(args.output/'cluster-diagnostic.jsonl',dict(arm=arm,result=result))
                assert not (result['status']=='failed' and 'stdout' in result),result
            legacy_replay(args.binary,args.output)
            protected_replay(args.binary,args.output)
        print('STAGED_CMG_SMOKE_PASS' , flush=True)
        return
    assert not rows.exists(), 'never overwrite prior observations'
    config = configuration()
    config['budget_seconds']=budget
    config['binary_sha256'] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    if args.protocol=='persistent':
        config['previous_binary_sha256']=hashlib.sha256(Path(os.environ['STAGED_PREVIOUS_BINARY']).read_bytes()).hexdigest()
        assert config['previous_binary_sha256']=='78313fb46e35b56da71a7261620b1852fdeca3beb6751babad2cbc0de967878f'
    (args.output / 'host.json').write_text(json.dumps(config, indent=2) + '\n')
    observer = Observer(args.output / 'host-observations.jsonl')
    observer.sample()
    observer.thread.start()
    rng = random.Random(seed)
    cells = manifest.copy()
    signatures = {}
    cpu = config['affinity'][0]
    started = time.monotonic()
    def check():
        observer.check()
        assert time.monotonic() - started < budget, 'campaign budget exceeded'
    try:
        if args.protocol in ['protected-grid','persistent','guarded-gate','router','admitted-gate','admitted-router','forecast-discovery','forecast-confirmation']:
            preflight(args.binary,args.output,cells,arms,cpu,check)
            if args.protocol in ROUTING_PROTOCOLS:
                for r in [json.loads(line) for line in (args.output/'preflight.jsonl').read_text().splitlines()]:
                    signatures[(r['case'],r['n'],r['rhs_count'],r['rhs_kind'],r['arm'])]=signature(r['result'])
        for block in ([-1] if args.mode == 'pilot' else range(-1, 6)):
            rng.shuffle(cells)
            for case, n, rhs, kind in cells:
                order = arms.copy()
                rng.shuffle(order)
                if block >= 0:
                    order += list(reversed(order))
                for arm in order:
                    check()
                    result = invoke(args.binary, case, n, rhs, arm, cpu, kind if kind in ['random','rev','components','components-rev'] else 'cut')
                    append(rows, {'phase': 'warmup' if block < 0 else 'measure', 'block': block,
                                  'case': case, 'n': n, 'rhs_count': rhs, 'rhs_kind': kind, 'arm': arm, 'result': result})
                    check()
                    if result['status'] == 'ok' or args.protocol in ROUTING_PROTOCOLS:
                        observed = signature(result)
                        key = (case, n, rhs, kind, arm)
                        assert signatures.setdefault(key, observed) == observed, 'repeat mismatch'
                print(f'block={block} case={case} rhs={rhs} kind={kind} complete', flush=True)
        observer.stop.set()
        observer.thread.join(timeout=45)
        assert not observer.thread.is_alive(), 'observer did not stop'
        observer.check()
        observer.sample()
        after = configuration()
        append(args.output / 'host-observations.jsonl', {'final_configuration': after})
        ignored = {'budget_seconds','binary_sha256', 'previous_binary_sha256', 'host_configuration_raw', 'complexes_raw'}
        assert {k: v for k, v in after.items() if k not in ignored} == {k: v for k, v in config.items() if k not in ignored}
        check()
        summarize(rows, args.output / 'summary.json', args.protocol)
        all_rows = [json.loads(line) for line in rows.read_text().splitlines()]
        expected=len(cells)*len(arms)*(1 if args.mode=='pilot' else 13)
        assert len(all_rows) == expected, 'incomplete protocol'
        failures = sum(r['result']['status'] != 'ok' for r in all_rows)
        marker='STAGED_CMG_PILOT_COMPLETE' if args.mode=='pilot' else 'STAGED_CMG_DISCOVERY_COMPLETE'
        print(f'{marker} observations={len(all_rows)} failures={failures}', flush=True)
    finally:
        observer.stop.set()


if __name__ == '__main__':
    main()
