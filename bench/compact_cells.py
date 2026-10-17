#!/usr/bin/env python3
"""Bounded, descriptive native-call discovery for the private compact-cell arm.

Prepare inputs and independent witnesses, run science, then measure a frozen
artifact in one isolated SCC allocation. No builds/oracle solves occur during
measurement. Every outcome remains on disk; a failure stops without resampling.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import random
import re
import socket
import statistics
import subprocess
import threading
import time

import numpy as np

import cmgschur
import cmgschur_cases as cases
import routing
import routing_cases
import routing_oracle
import routing_run
import scc_routing_host as scc
from native_io import digest

ARMS = ['baseline', 'cells', 'diagonal', 'cells_v2']
FAMILIES = ['weak', 'high-degree-weak', 'dense', 'repeated', 'nonmovers', 'hubs',
            'disconnected', 'weight-dynamic', 'wide', 'inactive-zero', 'low', 'easy']
LIMITS = {'seed': 2026101603, 'workers': [1, 4], 'blocks': 6, 'warmups': 1,
          'science_repeats': 2, 'call_timeout_s': 120, 'measurement_budget_s': 1800,
          'science_budget_s': 1800, 'warmup_budget_s': 1800, 'scheduler_timeout_s': 10,
          'process_period_s': 5, 'scheduler_period_s': 60,
          'process_maximum_gap_s': 15,
          'stopping': 'first failure or exhausted budget stops; no retries or replacements',
          'inference': 'descriptive point estimates only; no qualification or promotion',
          'exclusions': 'none; preserve every outcome and ordinary OS observation'}
LIBRARY_LIMITS = dict(scc.discovery.LIBRARY_LIMITS)
KIND = 'compact_cells_native_discovery_v1'
LIVE_HOST_LOAD_FIELDS = frozenset({
    'cpu', 'load_avg', 'load_short', 'load_medium', 'load_long',
    'np_load_avg', 'np_load_short', 'np_load_medium', 'np_load_long',
    'mem_free', 'mem_used', 'swap_free', 'swap_used', 'virtual_free', 'virtual_used', 'scratch_free',
})
STRUCTURAL_HOST_LOAD_FIELDS = frozenset({
    'arch', 'num_proc', 'mem_total', 'swap_total', 'virtual_total',
    'm_topology', 'm_topology_inuse', 'm_socket', 'm_core',
})


def require(condition, message):
    if not condition:
        raise ValueError(message)


def save(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def read(path):
    return json.loads(Path(path).read_text())


def ref(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': digest(path)}


def checked(value):
    path = Path(value['path']).resolve(strict=True)
    require(digest(path) == value['sha256'], 'frozen file changed: ' + str(path))
    return path


def host_fields(record):
    """Parse qconf host policy strictly, retaining live telemetry separately."""
    argv = record.get('argv')
    require(isinstance(argv, list) and len(argv) == 3 and argv[:2] == ['qconf', '-se']
            and isinstance(argv[2], str) and argv[2] and isinstance(record.get('stderr'), str),
            'invalid host configuration command metadata')
    text = scc.output(record, argv).replace('\\\n', '')
    require('\\' not in text, 'malformed host configuration continuation')
    fields = {}
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        parts = line.split(None, 1)
        require(len(parts) == 2 and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', parts[0]) is not None
                and parts[1].strip(), 'malformed host configuration field')
        key, value = parts[0], ' '.join(parts[1].split())
        require(key not in fields, 'duplicate host configuration field: ' + key)
        fields[key] = value

    def entries(value):
        if value == 'NONE':
            return {}
        result = {}
        for item in value.split(','):
            pair = [part.strip() for part in item.split('=')]
            require(len(pair) == 2 and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', pair[0]) is not None
                    and pair[1] and not any(c.isspace() for c in pair[1]),
                    'malformed host configuration key/value')
            key, entry = pair
            require(key not in result, 'duplicate host configuration key: ' + key)
            result[key] = entry
        return result

    require('load_values' in fields and 'complex_values' in fields, 'missing host load/configuration values')
    loads = entries(fields.pop('load_values'))
    fields['complex_values'] = entries(fields['complex_values'])
    require(STRUCTURAL_HOST_LOAD_FIELDS <= loads.keys(), 'missing structural host load values')
    for key in LIVE_HOST_LOAD_FIELDS & loads.keys():
        match = re.fullmatch(r'([+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)([bBkKmMgGtTpPeE]?)', loads[key])
        require(match is not None and math.isfinite(float(match[1]))
                and (not match[2] or key.endswith(('_free', '_used'))),
                'malformed live host telemetry: ' + key)
    return fields, loads


def check_stable_configuration(before, after):
    """Allow only declared load_values telemetry to vary; never alter raw rows."""
    for key in ['job_environment', 'parallel_environment', 'resource_quota']:
        require(before[key] == after[key], 'configuration changed: ' + key)
    left, right = before['queue_configuration'], after['queue_configuration']
    require(set(left) == set(right) == {'complexes', 'host', 'queue'}, 'malformed queue configuration record')
    for key in ['complexes', 'queue']:
        require(left[key] == right[key], 'configuration changed: ' + key)
    require({k: v for k, v in left['host'].items() if k != 'stdout'}
            == {k: v for k, v in right['host'].items() if k != 'stdout'},
            'host configuration command metadata changed')
    left_fields, left_loads = host_fields(left['host'])
    right_fields, right_loads = host_fields(right['host'])
    require(left_fields == right_fields and left_loads.keys() == right_loads.keys()
            and {k: v for k, v in left_loads.items() if k not in LIVE_HOST_LOAD_FIELDS}
            == {k: v for k, v in right_loads.items() if k not in LIVE_HOST_LOAD_FIELDS},
            'structural host configuration changed')


def tool_files():
    files = set(routing_run.tool_identities())
    files.update(Path(__file__).parent / name for name in
                 ['compact_cells.py', 'compact_cells_scc.sh', 'scc_routing_host.py',
                  'routing_discovery.py', 'block_host.py', 'tests/test_compact_cells.py'])
    return sorted(files)


def schedule(cell_ids):
    """Six complete blocks; each cell has a seeded Latin rotation over arms.

    The first four blocks balance every arm at every position; the remaining
    two preserve a maximum position-count difference of one. Cell order is
    independently shuffled in every block. Outcomes cannot alter this order.
    """
    rng = random.Random(LIMITS['seed'])
    initial = {}
    for cid in cell_ids:
        initial[cid] = rng.sample(ARMS, len(ARMS))
    blocks = []
    for block in range(LIMITS['blocks']):
        ids = rng.sample(list(cell_ids), len(cell_ids))
        slots = []
        for cid in ids:
            arms = initial[cid]
            shift = block % len(arms)
            slots.extend({'cell': cid, 'arm': arm} for arm in arms[shift:] + arms[:shift])
        blocks.append(slots)
    return blocks


def columns(family):
    seed = LIMITS['seed'] + FAMILIES.index(family)
    weights, intercept = None, True
    if family == 'high-degree-weak':
        values = cases.high_degree_case('d30-weak', 1100, 300, 32, seed)
    elif family in {'wide', 'low', 'inactive-zero'}:
        kind = 'orthogonal' if family == 'inactive-zero' else 'wide'
        values = cases.additional_case(kind, 1000, 100, 20, seed)
        intercept = family != 'inactive-zero'
    elif family in {'disconnected', 'weight-dynamic'}:
        values, weights, _, intercept = routing_cases.extended_columns(family, seed, 1000, 100, 20)
    else:
        values = cases.synthetic('weak' if family == 'weak' else 'short' if family == 'easy' else 'long',
                                 1000, 100, 4 if family == 'easy' else 20, seed)
        if family == 'dense':
            values = cases.high_degree_case('d30-mixed', 1000, 100, 32, seed)
        elif family == 'nonmovers':
            # Keep the first 100 connectivity workers, make all others degree one.
            values[-1][2000:] = np.repeat(np.arange(100, 1000) % 100, 20)
        elif family == 'hubs':
            values = cases.high_degree_case('d60-stress', 1000, 100, 64, seed)
        elif family == 'repeated':
            values = [np.repeat(value, 4) for value in values]
    require(len(values[0]) <= 100_000, 'case exceeds declared row bound')
    return values, weights, intercept


def prepare(root, generator, harness, plugin):
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=False)
    frozen = [ref(path) for path in [generator, harness, plugin, *tool_files()]]
    cells_out = []
    for family in FAMILIES:
        source = root / (family + '.case')
        values, weights, intercept = columns(family)
        cases.write_case(generator, source, values, include_intercept=intercept, weights=weights)
        if family == 'low':
            normal = source.with_suffix('.normal.case')
            source.rename(normal)
            routing_cases.copy_import(normal, source, low=True)
            normal.unlink()
        meta = routing_cases.inspect_case(source, family)
        meta.update(family=family, case_sha256=digest(source), **cases.graph_summary(values))
        metadata = source.with_suffix('.json')
        save(metadata, meta)
        witness = root / (family + '.oracle.json')
        routing_oracle.write_reference(source, witness)
        _, bindings = routing_oracle.load_reference(witness, meta['case_sha256'], meta['body_sha256'])
        frozen.extend(ref(path) for path in [source, metadata, *bindings])
        for workers in LIMITS['workers']:
            cid = f'{family}-t{workers}'
            inputs = {}
            for arm in ARMS:
                target = root / f'{cid}-{arm}.case'
                routing.prepare(source, target, arm, workers)
                require(routing_cases.inspect_case(target)['body_sha256'] == meta['body_sha256'],
                        'arm preparation changed frozen data/model body')
                inputs[arm] = ref(target)
                frozen.append(inputs[arm])
            cells_out.append({'id': cid, 'family': family, 'threads': workers,
                              'metadata': ref(metadata), 'witness': ref(witness), 'inputs': inputs})
    # Includes the producer closure, precomputed arrays, artifact and exact requests.
    identities = {item['path']: item['sha256'] for item in frozen}
    for path, sha in identities.items():
        require(digest(Path(path)) == sha, 'input/tool changed during preparation')
    plan = {'kind': KIND, 'limits': LIMITS, 'arms': ARMS, 'families': FAMILIES,
            'library_limits': LIBRARY_LIMITS, 'generator': ref(generator), 'harness': ref(harness),
            'plugin': ref(plugin), 'cells': cells_out, 'blocks': schedule([c['id'] for c in cells_out]),
            'frozen_files': identities, 'root': str(root), 'created_utc': datetime.now(timezone.utc).isoformat()}
    save(root / 'plan.json', routing.seal(plan))
    return plan


def validate(plan):
    require(plan['kind'] == KIND and plan['limits'] == LIMITS and plan['arms'] == ARMS
            and plan['families'] == FAMILIES and plan['library_limits'] == LIBRARY_LIMITS,
            'campaign contract changed')
    expected = [f'{f}-t{w}' for f in FAMILIES for w in LIMITS['workers']]
    require([c['id'] for c in plan['cells']] == expected and plan['blocks'] == schedule(expected),
            'case coverage or frozen order changed')
    for path, sha in plan['frozen_files'].items():
        checked({'path': path, 'sha256': sha})
    for path in tool_files():
        require(plan['frozen_files'].get(str(path.resolve())) == digest(path), 'unbound current tool')
    for cell in plan['cells']:
        require(set(cell['inputs']) == set(ARMS), 'incomplete arm coverage')
        for item in [cell['metadata'], cell['witness'], *cell['inputs'].values()]:
            require(plan['frozen_files'].get(item['path']) == item['sha256'], 'unbound case dependency')
    for key in ['plugin', 'harness', 'generator']:
        require(plan['frozen_files'].get(plan[key]['path']) == plan[key]['sha256'], 'unbound executable')


def native(plan, cell, arm, directory, label, *, traced=False, cpus=None, timeout=120):
    target = directory / (label + '.json')
    log = target.with_suffix('.log')
    command = routing_run.native_command(Path(plan['harness']['path']), Path(plan['plugin']['path']),
                                         Path(cell['inputs'][arm]['path']), target, cell, traced)
    if cpus is not None:
        command = ['taskset', '-c', ','.join(map(str, cpus)), *command]
    start = time.monotonic_ns()
    row = {'cell': cell['id'], 'arm': arm, 'label': label, 'target': str(target), 'log': str(log),
           'command': command, 'started_monotonic_ns': start, 'started_wall_ns': time.time_ns()}
    try:
        with log.open('xb') as stream:
            process = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=timeout,
                                     env={**os.environ, **LIBRARY_LIMITS})
        row.update(returncode=process.returncode, status='executed' if process.returncode == 0 else 'failed')
    except subprocess.TimeoutExpired:
        row.update(status='timeout', error='native call exceeded its frozen/deadline budget')
    except OSError as error:
        row.update(status='failed', error=str(error))
    row['completed_monotonic_ns'] = time.monotonic_ns()
    row['wall_seconds'] = (row['completed_monotonic_ns'] - start) / 1e9
    scc.append(directory / 'outcomes.jsonl', row)
    return row


def parse_run(row, meta):
    require(row['status'] == 'executed', 'native failure: ' + row['label'])
    target, log = Path(row['target']), Path(row['log'])
    raw, result = cmgschur.read_run(target, log, meta, 'auto')
    require(raw['return_code'] == 0, 'native estimator failure')
    return raw, result, target.with_suffix('.json.columns')


def science(plan, output, *, smoke=False):
    validate(plan)
    slots = int(os.environ.get('NSLOTS', '0'))
    require(os.environ.get('JOB_ID') and slots >= (2 if smoke else 4), 'science requires a compute allocation')
    output.mkdir(parents=True, exist_ok=False)
    selected = [plan['cells'][0]] if smoke else plan['cells']
    start = time.monotonic()
    fingerprints, records, baseline, worker_one = {}, [], {}, {}
    status, failure = 'passed', None
    try:
        for cell in selected:
            meta = read(cell['metadata']['path'])
            witness, _ = routing_oracle.load_reference(Path(cell['witness']['path']),
                                                        meta['case_sha256'], meta['body_sha256'])
            for arm in ARMS:
                previous = None
                for repeat in range(LIMITS['science_repeats']):
                    left = LIMITS['science_budget_s'] - (time.monotonic() - start)
                    require(left > 0, 'science budget exhausted')
                    row = native(plan, cell, arm, output, f"{cell['id']}-{arm}-{repeat}",
                                 traced=repeat == 0, timeout=min(LIMITS['call_timeout_s'], left))
                    records.append(row)
                    raw, result, binary = parse_run(row, meta)
                    routing_oracle.check_reference(witness, raw, binary, meta)
                    key = cell['id'] + '/' + arm
                    if previous is not None:
                        require(previous == result['science_sha256'], 'fixed-worker repeat differs: ' + key)
                    previous = result['science_sha256']
                    if repeat == 0:
                        row['resources'] = routing_run.resource_diagnostics(Path(row['log']).read_text(), arm,
                                                                            False, {})
                        if arm == 'baseline':
                            baseline[cell['id']] = (raw, binary)
                        base, base_columns = baseline[cell['id']]
                        cmgschur.scientific_comparison(base, raw, base_columns, binary, meta)
                        family_key = cell['family'] + '/' + arm
                        if cell['threads'] == 1:
                            worker_one[family_key] = (raw, binary)
                        else:
                            base, base_columns = worker_one[family_key]
                            cmgschur.scientific_comparison(base, raw, base_columns, binary, meta)
                    fingerprints[key] = result['science_sha256']
                    row['science_sha256'] = result['science_sha256']
        validate(plan)
        require(time.monotonic() - start <= LIMITS['science_budget_s'], 'science budget exhausted')
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
        status, failure = 'failed', str(error)
        if records and records[-1]['status'] == 'executed' and 'science_sha256' not in records[-1]:
            records[-1].update(status='failed', error=failure)
    bound = [ref(p) for p in output.iterdir() if p.is_file()]
    receipt = {'kind': KIND + '_science', 'status': status, 'failure': failure, 'smoke': smoke,
               'plan_sha256': routing.identity(plan), 'host': socket.gethostname(),
               'job_id': os.environ['JOB_ID'], 'slots': slots, 'fingerprints': fingerprints,
               'records': records, 'files': bound, 'seconds': time.monotonic() - start}
    save(output / 'science.json', routing.seal(receipt))
    return receipt


def validate_science(plan, receipt):
    require(receipt['kind'] == KIND + '_science' and receipt['status'] == 'passed'
            and receipt['smoke'] is False and receipt['plan_sha256'] == routing.identity(plan),
            'complete frozen science prerequisite required')
    require(set(receipt['fingerprints']) == {c['id'] + '/' + a for c in plan['cells'] for a in ARMS},
            'missing scientific prerequisite')
    for cell in plan['cells']:
        for arm in ARMS:
            rows = [r for r in receipt['records'] if r['cell'] == cell['id'] and r['arm'] == arm]
            require(len(rows) == LIMITS['science_repeats'] and all(r['status'] == 'executed'
                    and r.get('science_sha256') == receipt['fingerprints'][cell['id'] + '/' + arm]
                    for r in rows) and 'resources' in rows[0], 'incomplete repeat/resource prerequisite')
    for item in receipt['files']:
        checked(item)


class Observers:
    """Raw process samples and bounded scheduler censuses, without timing filters."""
    def __init__(self, directory, host, queue):
        self.directory, self.host, self.queue = directory, host, queue
        self.stop = threading.Event()
        self.errors = []
        self.threads = []
        self.census_lock = threading.Lock()
        self.sample_lock = threading.Lock()
        self.pid = os.getpid()

    def sample(self):
        with self.sample_lock:
            scc.append(self.directory / 'processes.jsonl', scc.process_snapshot())

    def census(self, stage):
        with self.census_lock:
            value = scc.scheduler(self.host, stage)
            scc.append(self.directory / 'scheduler.jsonl', value)
            scc.check_scheduler_snapshot(value, self.host, self.queue)

    def loop(self, period, action):
        next_at = time.monotonic() + period
        while not self.stop.wait(max(0, next_at - time.monotonic())):
            try:
                action()
            except Exception as error:
                # Parser/schema exceptions must not silently kill a worker.
                # SystemExit/KeyboardInterrupt remain outside ordinary errors.
                self.errors.append(str(error))
                scc.append(self.directory / 'observer-errors.jsonl',
                           {'wall_time_ns': time.time_ns(), 'type': type(error).__name__, 'error': str(error)})
                return
            next_at += period

    def start(self):
        self.sample()
        for period, action in [(LIMITS['process_period_s'], self.sample),
                               (LIMITS['scheduler_period_s'], lambda: self.census('periodic'))]:
            worker = threading.Thread(target=self.loop, args=(period, action), daemon=True)
            worker.start()
            self.threads.append(worker)

    def close(self):
        self.stop.set()
        try:
            self.sample()
        finally:
            for worker in self.threads:
                worker.join(timeout=25)
                require(not worker.is_alive(), 'observer did not finish within bounded command deadline')

    def validate(self, started, ended):
        require(not self.errors, 'isolation observer failed: ' + '; '.join(self.errors))
        scc.check_scheduler(self.directory / 'scheduler.jsonl', self.host, self.queue,
                            {'started_monotonic_ns': started, 'completed_monotonic_ns': ended})
        samples = list(scc.rows(self.directory / 'processes.jsonl'))
        require(len(samples) >= 2, 'missing process observation boundaries')
        previous = None
        for sample in samples:
            require(sample.get('kind') == 'snapshot', 'failed process observation')
            first, last = sample['monotonic_ns'], sample['finished_monotonic_ns']
            require(type(first) is int and type(last) is int and 0 < first <= last
                    and type(sample['wall_time_ns']) is int, 'invalid process sample timestamps')
            require(previous is None or 0 < first - previous <= LIMITS['process_maximum_gap_s'] * 1e9,
                    'process observation coverage gap')
            processes = scc.parsed_processes(sample)['processes']
            require(any(p['pid'] == self.pid and p['command'] is not None for p in processes),
                    'process observation lost its live collector identity')
            previous = first
        require(samples[0]['monotonic_ns'] <= started <= ended <= samples[-1]['finished_monotonic_ns'],
                'process samples do not bracket the measurement window')


def summarize(rows):
    """Only complete paired blocks; all failures remain explicit, never excluded."""
    groups = {}
    for row in rows:
        if row.get('phase') == 'measure':
            groups.setdefault((row['cell'], row['block']), {})[row['arm']] = row
    pairs = {}
    for (cid, block), arms in groups.items():
        if set(arms) != set(ARMS) or any(r.get('status') != 'passed' for r in arms.values()):
            continue
        baseline = arms['baseline']['seconds']
        for arm in ARMS[1:]:
            candidate = arms[arm]['seconds']
            pairs.setdefault((cid, arm), []).append({'block': block, 'baseline_seconds': baseline,
                'candidate_seconds': candidate, 'ratio': candidate / baseline,
                'allowance_excess_seconds': candidate - baseline - max(.05 * baseline, .001)})
    return [{'cell': cid, 'arm': arm, 'paired_blocks': len(values), 'pairs': values,
             'median_ratio': statistics.median(v['ratio'] for v in values),
             'maximum_ratio': max(v['ratio'] for v in values),
             'median_allowance_excess_seconds': statistics.median(v['allowance_excess_seconds'] for v in values),
             'maximum_allowance_excess_seconds': max(v['allowance_excess_seconds'] for v in values)}
            for (cid, arm), values in sorted(pairs.items())]


def measure(plan, receipt, output):
    validate(plan)
    validate_science(plan, receipt)
    require({k: os.environ.get(k) for k in LIBRARY_LIMITS} == LIBRARY_LIMITS,
            'set frozen library thread limits before starting measurement')
    host = scc.discovery.host()
    output.mkdir(parents=True, exist_ok=False)
    records, failure, observer, started, ended = [], None, None, None, None
    before, queue, observed_started = None, None, None
    fingerprints = {}
    try:
        before, snapshot = scc.configuration(output, host, 'before')
        queue = scc.check_configuration(before, host, os.getpid())
        scc.check_scheduler_snapshot(snapshot, host, queue)
        # The validator proves all allowed CPUs are distinct physical cores.
        cpus = sorted(host['affinity'])[:4]
        observer = Observers(output, host, queue)
        observer.start()
        observed_started = time.monotonic_ns()
        by_id = {c['id']: c for c in plan['cells']}
        # One fixed warmup per exact request, in this same allocation/affinity.
        warmup_started = time.monotonic()
        for cell in plan['cells']:
            for arm in ARMS:
                require(not observer.errors, 'isolation observer failed')
                remaining = LIMITS['warmup_budget_s'] - (time.monotonic() - warmup_started)
                require(remaining > 0, 'warmup budget exhausted')
                row = native(plan, cell, arm, output, f"warmup-{cell['id']}-{arm}", cpus=cpus,
                             traced=True, timeout=min(LIMITS['call_timeout_s'], remaining))
                row.update(phase='warmup')
                records.append(row)
                meta = read(cell['metadata']['path'])
                raw, result, binary = parse_run(row, meta)
                original = next(r for r in receipt['records'] if r['cell'] == cell['id'] and r['arm'] == arm)
                reference, _, reference_columns = parse_run(original, meta)
                cmgschur.scientific_comparison(reference, raw, reference_columns, binary, meta)
                row['resources'] = routing_run.resource_diagnostics(Path(row['log']).read_text(), arm, False, {})
                fingerprints[cell['id'] + '/' + arm] = result['science_sha256']
                row.update(result, status='passed')
        require(time.monotonic() - warmup_started <= LIMITS['warmup_budget_s'], 'warmup budget exhausted')
        started = time.monotonic_ns()
        for index, block in enumerate(plan['blocks']):
            observer.census(f'block-{index}-before')
            for slot in block:
                require(not observer.errors, 'isolation observer failed')
                remaining = LIMITS['measurement_budget_s'] - (time.monotonic_ns() - started) / 1e9
                require(remaining > 0, 'measurement budget exhausted')
                cell, arm = by_id[slot['cell']], slot['arm']
                row = native(plan, cell, arm, output, f"block-{index}-{cell['id']}-{arm}", cpus=cpus,
                             timeout=min(LIMITS['call_timeout_s'], remaining))
                row.update(phase='measure', block=index)
                records.append(row)
                # Verification between complete calls stops the campaign at its
                # first scientific failure. This never runs an oracle solve.
                _, result, _ = parse_run(row, read(cell['metadata']['path']))
                require(result['science_sha256'] == fingerprints[cell['id'] + '/' + arm],
                        'measured result changed same-allocation fixed-worker science')
                row.update(result, status='passed')
            observer.census(f'block-{index}-after')
        ended = time.monotonic_ns()
        require((ended - started) / 1e9 <= LIMITS['measurement_budget_s'], 'measurement budget exhausted')
    except Exception as error:
        failure = str(error)
        if records and records[-1]['status'] == 'executed':
            records[-1].update(status='failed', error=failure)
    finally:
        ended = ended or time.monotonic_ns()
        # Join before appending final scheduler state; periodic and boundary
        # snapshots must retain their physical chronological order.
        if observer:
            try:
                observer.close()
                require(not observer.errors, 'observer failed: ' + '; '.join(observer.errors))
            except Exception as error:
                failure = failure or str(error)
        if before is not None and queue is not None:
            try:
                after, snapshot = scc.configuration(output, host, 'after')
                require(scc.check_configuration(after, host, os.getpid()) == queue, 'allocation changed')
                scc.check_scheduler_snapshot(snapshot, host, queue)
                check_stable_configuration(before, after)
                if observer and observed_started is not None:
                    observer.validate(started if started is not None else observed_started, ended)
            except Exception as error:
                failure = failure or str(error)
    try:
        validate(plan)
        validate_science(plan, receipt)
    except (ValueError, OSError) as error:
        failure = failure or str(error)
    value = {'kind': KIND + '_measurement', 'plan_sha256': routing.identity(plan),
             'science_sha256': routing.identity(receipt), 'status': 'failed' if failure else 'complete',
             'failure': failure, 'host': host, 'collection_pid': os.getpid(),
             'native_affinity': sorted(host['affinity'])[:4], 'started_monotonic_ns': started,
             'completed_monotonic_ns': ended, 'records': records, 'summaries': summarize(records),
             'files': [ref(p) for p in output.iterdir() if p.is_file()],
             'claim': LIMITS['inference'], 'accounting': 'pending external qacct validation',
             'process_evidence': 'raw observations retained; competing-compute review required; no exclusions'}
    save(output / 'measurement.json', routing.seal(value))
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    prep = subs.add_parser('prepare')
    for name in ['root', 'generator', 'harness', 'plugin']:
        prep.add_argument('--' + name, type=Path, required=True)
    for mode in ['science', 'smoke', 'measure']:
        sub = subs.add_parser(mode)
        sub.add_argument('--plan', type=Path, required=True)
        sub.add_argument('--output', type=Path, required=True)
        if mode == 'measure':
            sub.add_argument('--science', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.root, args.generator.resolve(strict=True), args.harness.resolve(strict=True),
                args.plugin.resolve(strict=True))
    else:
        plan = routing.unseal(read(args.plan))
        result = (measure(plan, routing.unseal(read(args.science)), args.output) if args.command == 'measure'
                  else science(plan, args.output, smoke=args.command == 'smoke'))
        require(result['status'] in {'passed', 'complete'}, result.get('failure') or 'campaign failed')
    print(f'COMPACT_CELLS_SUCCESS stage={args.command}', flush=True)


if __name__ == '__main__':
    main()
