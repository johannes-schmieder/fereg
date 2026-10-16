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
    argv = [str(binary), case, str(n), str(rhs), arm, pattern]
    if cpu is not None:
        argv = ['taskset', '-c', str(cpu)] + argv
    try:
        run = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        result = json.loads(run.stdout)
        if run.returncode != 0:
            result['status'] = 'failed'
            result['exit_status'] = run.returncode
        if result['status'] == 'ok':
            assert (result['case'], result['n'], result['rhs'], result['arm']) == (case, n, rhs, arm)
            assert math.isfinite(result['seconds']) and result['seconds'] > 0
            assert 0 <= result['relative_residual'] <= 1e-8
            assert result['graph_builds'] <= 1
        return result
    except subprocess.TimeoutExpired:
        return {'status': 'timeout'}
    except (ValueError, AssertionError, KeyError, TypeError) as error:
        return {'status': 'failed', 'reason': str(error), 'stdout': run.stdout, 'stderr': run.stderr}


def summarize(path, output):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    results = []
    for case in CASES:
        for rhs, kind in [(1, 'cut'), (1, 'random'), (4, 'mixed')]:
            cell = [r for r in rows if r['case'] == case and r['rhs_count'] == rhs and r['rhs_kind'] == kind and r['phase'] == 'measure']
            for arm in ARMS:
                arm_rows = [r for r in cell if r['arm'] == arm]
                ratios = []
                for block in range(6):
                    a = [r['result'] for r in arm_rows if r['block'] == block]
                    b = [r['result'] for r in cell if r['arm'] == 'diagonal' and r['block'] == block]
                    if len(a) == len(b) == 2 and all(r['status'] == 'ok' for r in a+b):
                        ratios.append(math.exp(sum(math.log(r['seconds']) for r in a)/2
                                               - sum(math.log(r['seconds']) for r in b)/2))
                success = [r['result'] for r in arm_rows if r['result']['status'] == 'ok']
                results.append({'case': case, 'rhs': rhs, 'rhs_kind': kind, 'arm': arm,
                    'observations': len(arm_rows), 'failures': len(arm_rows)-len(success),
                    'block_ratios_to_diagonal': ratios,
                    'median_ratio': statistics.median(ratios) if len(ratios) == 6 else None,
                    'mean_seconds': statistics.mean(r['seconds'] for r in success) if success else None,
                    'mean_setup_seconds': statistics.mean(r['setup_seconds'] for r in success) if success else None,
                    'iterations': sorted({r['iterations'] for r in success}),
                    'upgrades': sorted({r['upgrades'] for r in success}),
                    'digests': sorted({r['digest'] for r in success})})
    output.write_text(json.dumps({'scope': 'serial prepared-Schur SCC discovery only',
        'blocks': 6, 'seed': SEED, 'results': results}, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['smoke', 'measure', 'summary'])
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = args.output / 'observations.jsonl'
    if args.mode == 'summary':
        summarize(rows, args.output / 'summary.json')
        return
    assert args.binary and args.binary.is_file()
    if args.mode == 'smoke':
        for case in CASES:
            for arm in ARMS:
                result = invoke(args.binary, case, 256, 2, arm)
                append(args.output / 'smoke.jsonl', {'case': case, 'arm': arm, 'result': result})
                assert result['status'] == 'ok', (case, arm, result)
        print('STAGED_CMG_SMOKE_PASS', flush=True)
        return
    assert not rows.exists(), 'never overwrite prior observations'
    config = configuration()
    config['binary_sha256'] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    (args.output / 'host.json').write_text(json.dumps(config, indent=2) + '\n')
    observer = Observer(args.output / 'host-observations.jsonl')
    observer.sample()
    observer.thread.start()
    rng = random.Random(SEED)
    cells = [(c, rhs, kind) for c in CASES for rhs, kind in [(1, 'cut'), (1, 'random'), (4, 'mixed')]]
    signatures = {}
    cpu = config['affinity'][0]
    started = time.monotonic()
    try:
        for block in range(-1, 6):
            rng.shuffle(cells)
            for case, rhs, kind in cells:
                order = ARMS.copy()
                rng.shuffle(order)
                if block >= 0:
                    order += list(reversed(order))
                for arm in order:
                    observer.check()
                    assert time.monotonic() - started < 1800, 'campaign budget exceeded'
                    result = invoke(args.binary, case, 4096, rhs, arm, cpu, 'random' if kind == 'random' else 'cut')
                    append(rows, {'phase': 'warmup' if block < 0 else 'measure', 'block': block,
                                  'case': case, 'rhs_count': rhs, 'rhs_kind': kind, 'arm': arm, 'result': result})
                    if result['status'] == 'ok':
                        signature = (result['digest'], result['iterations'], result['upgrades'])
                        key = (case, rhs, kind, arm)
                        assert signatures.setdefault(key, signature) == signature, 'repeat mismatch'
                print(f'block={block} case={case} rhs={rhs} kind={kind} complete', flush=True)
        observer.stop.set()
        observer.thread.join(timeout=45)
        assert not observer.thread.is_alive(), 'observer did not stop'
        observer.check()
        observer.sample()
        after = configuration()
        append(args.output / 'host-observations.jsonl', {'final_configuration': after})
        ignored = {'binary_sha256', 'host_configuration_raw', 'complexes_raw'}
        assert {k: v for k, v in after.items() if k not in ignored} == {k: v for k, v in config.items() if k not in ignored}
        summarize(rows, args.output / 'summary.json')
        all_rows = [json.loads(line) for line in rows.read_text().splitlines()]
        failures = sum(r['result']['status'] != 'ok' for r in all_rows)
        print(f'STAGED_CMG_DISCOVERY_COMPLETE observations={len(all_rows)} failures={failures}', flush=True)
    finally:
        observer.stop.set()


if __name__ == '__main__':
    main()
