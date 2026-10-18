#!/usr/bin/env python3
"""One fixed-candidate regression round: plan, parallel runs, and one report.

Plans contain the actual binaries and fixtures, not previous campaign proofs.
Run all arms together; check independent science outside the native timer.
Failures require discussion and never select another candidate or launch work.
"""
from __future__ import annotations

import argparse
import copy
from contextlib import contextmanager
from fractions import Fraction
import json
import importlib
import math
import os
from pathlib import Path
import platform
import random
import re
import signal
import socket
import statistics
import sys
import threading
import time

import numpy as np

import cmgschur
import routing
import routing_cases
import routing_oracle
import routing_science as science
from native import run_process
from native_io import ROOT, case_header, diagnostics, digest, numbers, prepare_case, validate_result
from replay_plugins import exact_view

FAMILIES = {'no-fe', 'one-fe', 'slopes', 'low-saved', *routing_cases.NATIVE_CASES,
            *routing_cases.SYNTHETIC_CASES, *routing_cases.EXTENDED_CASES,
            *routing_cases.SAMPLED_CASES}
ENVIRONMENT = ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING',
               'FEREG_CMG_ORDINARY', 'FEREG_ACTION_MEMORY_AUDIT', 'FEREG_PPML_MEMORY_TRACE',
               'FEREG_MEMORY_LIMIT_BYTES', 'LLVM_PROFILE_FILE')
THREAD_ENV = ('OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS', 'MKL_NUM_THREADS',
              'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS')
ALPHA = Fraction(1, 40)
SOURCE_MODULES = ('qualify_scc', 'block_host', 'native', 'native_io', 'routing',
                  'routing_cases', 'routing_oracle', 'routing_science', 'cmgschur',
                  'cmgschur_cases', 'time_exact', 'replay_plugins', 'replay_psd',
                  'replay_bias_reference', 'replay_cmg_polish', 'replay_ols_reference')


def execution_sources():
    """Only maintained execution/check code, not an environment or history walk."""
    return {'qualify': Path(__file__).resolve(), **{name: Path(importlib.import_module(name).__file__).resolve()
                                                for name in SOURCE_MODULES}}


def verify_sources(expected):
    paths = execution_sources()
    require(set(expected) == set(paths), 'execution source coverage changed; make a new plan')
    identities = {}
    for name, path in paths.items():
        before = stamp(path)
        require(digest(path) == expected[name] and stamp(path) == before,
                'execution source identity mismatch: ' + name)
        identities[path] = before
    return identities


class BudgetExpired(TimeoutError):
    pass


class Deadline:
    """Main-thread interrupt, with uninterrupted owned-process cleanup on failure."""
    def __init__(self, seconds, started):
        self.seconds, self.started = seconds, started
        self.expired = False

    def check(self):
        left = self.seconds - (time.monotonic() - self.started)
        if self.expired or left <= 0:
            self.expired = True
            raise BudgetExpired('fixed shard wall-clock budget exhausted')
        return left

    def cleanup(self):
        signal.setitimer(signal.ITIMER_REAL, 0)
        for sig in self.handlers:
            signal.signal(sig, signal.SIG_IGN)

    def interrupt(self, sig, _frame):
        self.cleanup()  # Never interrupt native kill/reap or observer shutdown twice.
        if sig == signal.SIGALRM:
            self.expired = True
            raise BudgetExpired('fixed shard wall-clock budget exhausted')
        raise KeyboardInterrupt('qualification interrupted')

    def arm(self):
        left = self.check()
        for sig in self.handlers:
            signal.signal(sig, self.interrupt)
        signal.setitimer(signal.ITIMER_REAL, left)

    def __enter__(self):
        require(threading.current_thread() is threading.main_thread(), 'run requires the main thread')
        require(signal.getitimer(signal.ITIMER_REAL) == (0.0, 0.0), 'run requires an unused real-time timer')
        self.handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGALRM, signal.SIGINT, signal.SIGTERM)}
        self.arm()
        return self

    def __exit__(self, *_):
        signal.setitimer(signal.ITIMER_REAL, 0)
        for sig, handler in self.handlers.items():
            signal.signal(sig, handler)

    @contextmanager
    def observing(self, host):
        host.__enter__()
        try:
            yield host
        except BaseException:
            self.cleanup()
            host.__exit__(*sys.exc_info())
            raise
        else:
            # Observer commands already have fixed timeouts. Let them reap their
            # children, then reject any shutdown which exhausted the run budget.
            self.cleanup()
            host.__exit__(None, None, None)
            self.arm()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.pending')
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')
    temporary.replace(path)


def read(path):
    return routing.read_json(Path(path))


def stamp(path):
    stat = path.stat()
    return stat.st_dev, stat.st_ino, stat.st_mode, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns


def resolve(manifest_path, path):
    return (Path(manifest_path).parent / path).resolve(strict=True)


def plan(config_path, output):
    """Hash each actual input once. No native call, oracle solve, or history walk."""
    config_path, output = Path(config_path).resolve(), Path(output).resolve()
    require(not output.exists(), 'plan already exists')
    value = read(config_path)
    required = {'scope', 'platform', 'artifacts', 'harness', 'cases', 'threads',
                'blocks', 'shards', 'replicas', 'seed', 'timeout_s', 'budget_s', 'coverage'}
    require(set(value) == required, 'configuration needs exactly: ' + ', '.join(sorted(required)))
    require(value['scope'] in {'exact', 'many', 'two'}, 'unsupported numerical scope')
    require(value['platform'] in {'Darwin', 'Linux'}, 'platform must be Darwin or Linux')
    require(value['coverage'] in {'routing', 'smoke'}, 'coverage must be routing or smoke')
    for field, low, high in [('blocks', 1, 24), ('shards', 1, 42), ('replicas', 1, 12),
                             ('seed', 0, 2**64 - 1), ('timeout_s', 1, 120), ('budget_s', 1, 14400)]:
        require(type(value[field]) is int and low <= value[field] <= high, 'invalid ' + field)
    workers = value['threads']
    require(isinstance(workers, list) and workers and len(set(workers)) == len(workers)
            and all(type(n) is int and 1 <= n <= 1024 for n in workers), 'invalid workers')
    require(value['platform'] != 'Darwin' or value['replicas'] == value['shards'] == 1,
            'Mac timing uses one serial host run')
    artifacts = value['artifacts']
    require(set(artifacts) in ({'original', 'incumbent', 'candidate'},
                              {'original', 'incumbent', 'candidate', 'hidden'}), 'required artifact roles missing')
    files = {}

    def bind(name):
        path = (config_path.parent / name).resolve(strict=True)
        key = os.path.relpath(path, output.parent)
        if key not in files:
            files[key] = digest(path)
        return key

    for role, artifact in artifacts.items():
        require(set(artifact) == {'path', 'revision', 'build', 'request', 'families'}, 'invalid artifact ' + role)
        require(isinstance(artifact['revision'], str) and artifact['revision'], 'record artifact revision')
        require(isinstance(artifact['build'], str) and artifact['build'], 'record compiler and build settings')
        require(artifact['request'] in {'auto', 'preserve', *routing.INCREMENTAL}, 'unsupported request')
        require(isinstance(artifact['families'], list) and set(artifact['families']) <= {'many', 'two'},
                'invalid artifact numerical families')
        artifact['path'] = bind(artifact['path'])
    require(artifacts['original']['families'] == [], 'original baseline must remain original')
    require(all(artifacts[r]['request'] in {'auto', 'preserve'} for r in ('original', 'incumbent')),
            'protected references must use their public requests')
    require(set(artifacts['candidate']['families']) == (set(artifacts['incumbent']['families']) |
            ({value['scope']} if value['scope'] != 'exact' else set())), 'candidate changes an undeclared family')
    require(value['scope'] == 'exact' or value['scope'] in artifacts['candidate']['families'],
            'candidate lacks the declared changed family')
    require(value['scope'] == 'exact' or artifacts['candidate']['request'] != 'preserve',
            'changed Auto qualification requires an explicit auto or incremental request')
    if value['scope'] != 'exact' and artifacts['candidate']['request'] == 'auto':
        require('hidden' in artifacts and artifacts['hidden']['request'] == 'incremental-' + value['scope'],
                'public activation requires the selected hidden artifact')
    value['harness'] = bind(value['harness'])
    seen = set()
    require(isinstance(value['cases'], list) and value['cases'], 'cases must be a nonempty list')
    for item in value['cases']:
        require(set(item) == {'id', 'family', 'input', 'oracle', 'memory_limit_bytes'}, 'invalid case fields')
        require(re.fullmatch(r'[a-zA-Z0-9_-]+', item['id']) and item['id'] not in seen, 'invalid/duplicate case id')
        seen.add(item['id'])
        require(item['memory_limit_bytes'] is None or type(item['memory_limit_bytes']) is int
                and item['memory_limit_bytes'] > 0, 'invalid memory bound')
        source = (config_path.parent / item['input']).resolve(strict=True)
        item['input'] = bind(item['input'])
        with source.open('rb') as stream:
            shape, arguments = case_header(stream)
        require(not any(a.startswith((b'inject=', b'break_marker_token=')) for a in arguments),
                'source contains developer instrumentation')
        require(arguments[0] == b'estimate-ols', 'routing qualification is OLS; PPML uses the mapped feature/replay suite')
        item['shape'] = shape[:2]
        item['metadata'] = None
        sidecar = source.with_suffix('.json')
        if sidecar.exists():
            item['metadata'] = bind(str(sidecar))
            require(value['coverage'] == 'smoke' or not read(sidecar).get('small', False),
                    'small tooling fixtures cannot qualify routing performance')
        oracle = item['oracle']
        require(isinstance(oracle, dict) and oracle.get('kind') in {'routing', 'replay', 'dense', 'inactive'},
                'independent numerical reference is required')
        if oracle['kind'] in {'routing', 'replay'}:
            require(set(oracle) == {'kind', 'path'}, 'invalid oracle fields')
            path = (config_path.parent / oracle['path']).resolve(strict=True)
            oracle['path'] = bind(str(path))
            if oracle['kind'] == 'routing':
                reference = read(path)
                oracle['arrays'] = bind(str(path.parent / reference['arrays']['file']))
        else:
            require(set(oracle) == {'kind'}, 'invalid inline oracle')
        if value['scope'] != 'exact':
            routing_cases.inspect_case(source, item['family'])
            options = dict(a.decode().split('=', 1) for a in arguments[1:])
            require(options.get('solver') == options.get('preconditioner') == 'auto',
                    'routing qualification requires source Auto/Auto')
    require(value['cases'] and value['shards'] <= len(value['cases']), 'empty case shard')
    if value['coverage'] == 'routing':
        require(len(value['cases']) == len(FAMILIES) and {c['family'] for c in value['cases']} == FAMILIES
                and sorted(workers) == [1, 4], 'routing qualification requires all 42 workload cells')
        require(value['blocks'] % 2 == 0 and value['blocks'] >= (6 if value['platform'] == 'Darwin' else 2),
                'qualification requires balanced blocks and enough independent measurements')
    value.update(schema=1, files=files, warmups=1,
                 execution_sources={name: digest(path) for name, path in execution_sources().items()})
    output.parent.mkdir(parents=True, exist_ok=True)
    write(output, value)
    return value


def metadata(manifest, item, manifest_path):
    source = resolve(manifest_path, item['input'])
    meta = read(resolve(manifest_path, item['metadata'])) if item['metadata'] else {}
    meta.update(routing_cases.inspect_case(source, item['family']))
    oracle = item['oracle']
    if oracle['kind'] == 'routing':
        meta['routing_oracle'], _ = routing_oracle.load_reference(
            resolve(manifest_path, oracle['path']), manifest['files'][item['input']], meta['body_sha256'])
    elif oracle['kind'] == 'replay':
        meta['replay_reference'] = read(resolve(manifest_path, oracle['path']))
    elif oracle['kind'] == 'inactive':
        require(item['family'] == 'inactive-columns', 'inactive oracle has wrong family')
        meta['oracle'] = routing_cases.inactive_case_oracle(source)
    require(any(key in meta for key in ('routing_oracle', 'oracle', 'replay_reference')), 'missing independent truth')
    return meta


def continuation_accounting(old, previous):
    """Debit started slots, never select or reuse previous timing observations."""
    require('continuation' not in old and old['platform'] == 'Darwin'
            and old['schema'] == old['shards'] == old['replicas'] == old['warmups'] == 1,
            'requires an original serial Mac plan')
    require(type(old['blocks']) is int and 6 <= old['blocks'] <= 24 and old['blocks'] % 2 == 0
            and type(old['budget_s']) is int and 1 <= old['budget_s'] <= 14400
            and type(old['timeout_s']) is int and 1 <= old['timeout_s'] <= 120, 'invalid original fixed budgets')
    require(previous['manifest_sha256'] == routing.identity(old) and previous['status'] == 'blocked'
            and previous['shard'] == previous['replica'] == 0 and previous['platform'] == 'Darwin'
            and not previous.get('previous_hosts') and len(previous['failures']) == 1
            and previous['failures'][0]['kind'] == 'operational', 'requires one operational failure')
    require(previous.get('budget_compliant') is True and previous.get('cleanup_overrun_s') == 0,
            'prior run did not comply with its budget')
    require(previous['execution_sources'] == old['execution_sources']
            and set(old['execution_sources']) == set(execution_sources())
            and all(routing.sha(s) for s in old['execution_sources'].values()), 'prior source vector mismatch')
    host = previous['host']
    require(previous['hostname'] == host['host']['hostname'] and host['host']['platform'] == 'Darwin'
            and 0 < host['started_monotonic_ns'] <= host['finished_monotonic_ns'], 'invalid prior host')
    elapsed = previous['elapsed_s']
    require(math.isfinite(elapsed) and 0 <= elapsed < old['budget_s'], 'prior budget exhausted')
    aliases = artifact_aliases(old)
    arms = list(dict.fromkeys(aliases.values()))
    cells = [(f"{item['id']}-t{t}", i, item['family']) for i, item in enumerate(old['cases']) for t in old['threads']]
    require(len({c[0] for c in cells}) == len(cells), 'duplicate planned cells')
    if old['coverage'] == 'routing':
        require(len(cells) == 42 and {c['family'] for c in old['cases']} == FAMILIES
                and sorted(old['threads']) == [1, 4], 'continuation requires all 42 cells')
    prior = previous['cases']
    require([c['id'] for c in prior] == [c[0] for c in cells[:len(prior)]]
            and len(prior) <= len(cells), 'missing, duplicate or reordered prior cells')
    starts, calls = dict.fromkeys((c[0] for c in cells), 0), 0
    for n, cell in enumerate(prior):
        cid, index, family = cells[n]
        require(cell['family'] == family and cell['aliases'] == aliases and cell['host_attempt'] == 0
                and cell['status'] in {'complete', 'running'}
                and (n == len(prior)-1 or cell['status'] == 'complete'), 'invalid prior cell')
        warmups = cell['warmups']
        require([c['execution'] for c in warmups] == arms[:len(warmups)] and len(warmups) <= len(arms),
                'missing, duplicate or reordered warmups')
        rows = cell['blocks']
        require(len(rows) <= old['blocks'] and (not rows or len(warmups) == len(arms))
                and (cell['status'] != 'complete' or len(rows) == old['blocks']), 'missing prior blocks')
        expected = orders(arms, old['blocks'], old['seed'] + index)
        fingerprints = {c['execution']: c['fingerprint'] for c in warmups}
        for b, row in enumerate(rows):
            require(row['index'] == b and row['order'] == expected[b]
                    and [c['execution'] for c in row['calls']] == expected[b][:len(row['calls'])]
                    and len(row['calls']) <= len(expected[b]) and row['status'] in {'complete', 'running'}
                    and (row['status'] == 'complete' or b == len(rows)-1 and cell['status'] == 'running')
                    and (row['status'] != 'complete' or len(row['calls']) == len(expected[b]))
                    and all(c['fingerprint'] == fingerprints[c['execution']] for c in row['calls']),
                    'missing, duplicate or reordered prior calls/blocks')
        starts[cid] = len(rows)
        calls += len(warmups) + sum(len(r['calls']) for r in rows)
    indices = {cid: list(range(start, old['blocks'])) for cid, start in starts.items()}
    require(all(start % 2 == 0 for start in starts.values()), 'started slots leave an unbalanced orientation pair')
    require(all(len(v) >= 6 for v in indices.values()), 'fewer than six untouched blocks remain')
    warmups, timed = len(cells)*len(arms), sum(map(len, indices.values()))*2*len(arms)
    cap = len(cells)*len(arms)*(1+2*old['blocks'])
    require(calls+warmups+timed <= cap, 'continuation would increase original native-call budget')
    budget = math.floor(old['budget_s']-elapsed)
    require(budget >= 1, 'no whole seconds remain')
    return {'hostname': previous['hostname'], 'prior_execution_sources': old['execution_sources'],
            'prior_elapsed_s': elapsed, 'prior_calls': calls, 'starting_slots': starts, 'block_indices': indices,
            'prior_complete_blocks': sum(r['status'] == 'complete' for c in prior for r in c['blocks']),
            'prior_partial_blocks': sum(r['status'] == 'running' for c in prior for r in c['blocks']),
            'warmup_calls': warmups, 'timed_calls': timed, 'original_call_cap': cap, 'budget_s': budget}


def continuation_plan(old_path, result_path, output, publish=True):
    old_path, result_path, output = map(lambda p: Path(p).resolve(), (old_path, result_path, output))
    require(not publish or not output.exists(), 'continuation plan already exists')
    old, previous = read(old_path), read(result_path)
    state = continuation_accounting(old, previous)
    # Keep unchanged relative bindings; relocation is unnecessary for one local round.
    require(output.parent == old_path.parent, 'continuation plan must share the original plan directory')
    if publish:
        require(state['hostname'] == socket.gethostname(), 'continuation requires the original host')
        for name, expected in old['files'].items():
            path = resolve(old_path, name)
            before = stamp(path)
            require(digest(path) == expected and stamp(path) == before, 'input identity mismatch: '+name)
    value = copy.deepcopy(old)
    value.update(budget_s=state['budget_s'])
    if publish:
        value['execution_sources'] = {name: digest(path) for name, path in execution_sources().items()}
    value['continuation'] = {**state, 'prior_plan': {'path': old_path.name, 'sha256': digest(old_path)},
                             'prior_result': {'path': os.path.relpath(result_path, output.parent), 'sha256': digest(result_path)}}
    if publish:
        write(output, value)
    return value


def validate_continuation(manifest, path):
    if 'continuation' in manifest:
        state = manifest['continuation']
        prior = [resolve(path, state[key]['path']) for key in ('prior_plan', 'prior_result')]
        require(all(digest(p) == state[key]['sha256'] for p, key in zip(prior, ('prior_plan', 'prior_result'))),
                'prior continuation evidence changed')
        expected = continuation_plan(*prior, path, publish=False)
        expected['execution_sources'] = manifest['execution_sources']  # Independently verified once by run/report.
        require(manifest == expected, 'continuation schedule, inputs, sources or budget changed')


def continuation_bindings(manifest, path):
    state = manifest.get('continuation')
    return {state[key]['path']: resolve(path, state[key]['path']) for key in ('prior_plan', 'prior_result')} if state else {}


def block_indices(manifest, cell_id):
    return manifest['continuation']['block_indices'][cell_id] if 'continuation' in manifest else range(manifest['blocks'])


def read_native(target, log, meta, check_reference=True):
    raw = read(target)
    require(raw.pop('columns') == [], 'native harness must write binary columns')
    validate_result(raw)
    detail = diagnostics(raw, False)
    match = re.findall(r'^NATIVE_SECONDS=([0-9.]+)$', log.read_text(), re.MULTILINE)
    require(len(match) == 1 and math.isfinite(float(match[0])) and float(match[0]) > 0,
            'missing complete native-call timer')
    columns = target.with_suffix('.json.columns')
    require(columns.stat().st_size == 8 * meta['rows'] * meta['columns'], 'native column shape mismatch')
    signature = {**exact_view(raw, False), 'columns_sha256': digest(columns)}
    if check_reference:
        science.check_oracle(raw, columns, meta)
    return raw, columns, {**detail, 'seconds': float(match[0]), 'fingerprint': routing.identity(signature)}


def worker_signature(raw):
    solver = numbers(raw['matrices']['RSOLVER'])
    return routing.identity({'model': science.model_discrete_sha256(raw),
        'solver': [[solver[16 * row + i] for i in (0, 1, 3, 10, 11, 12, 13, 14)] for row in range(2)]})


def affected(meta, families):
    return any(routing.incremental_eligible(meta, family) for family in families)


def artifact_aliases(manifest):
    keys, aliases = {}, {}
    for role, artifact in manifest['artifacts'].items():
        key = (manifest['files'][artifact['path']], artifact['request'])
        aliases[role] = keys.setdefault(key, role)
    return aliases


def compare_artifacts(manifest, meta, snapshots):
    candidate = snapshots['candidate']
    for role in ('original', 'incumbent', 'hidden'):
        if role not in snapshots:
            continue
        reference = snapshots[role]
        exact = (role == 'hidden' or manifest['scope'] == 'exact'
                 or role == 'incumbent' and not affected(meta, [manifest['scope']])
                 or not affected(meta, manifest['artifacts']['candidate']['families']))
        if exact:
            require(candidate[2]['fingerprint'] == reference[2]['fingerprint'],
                    role + ': unaffected science or selected-hidden equivalence changed')
        else:
            science.compatibility(reference[0], candidate[0], reference[1], candidate[1], meta, 'auto',
                                  selected_policy=routing.policy(routing.PORTFOLIO_UNSEEDED))


def orders(arms, blocks, seed):
    rng = random.Random(seed)
    result = []
    for index in range(blocks):
        if index % 2 == 0:
            order = list(arms)
            rng.shuffle(order)
        else:
            order = list(reversed(order))
        result.append(order + list(reversed(order)))
    return result


def interval(values):
    require(values and all(math.isfinite(v) for v in values), 'invalid independent scores')
    ordered, n = sorted(values), len(values)
    denominator, tail, rank = 2**n, 2**n, None
    for k in range(1, n + 1):
        tail -= math.comb(n, k - 1)
        if Fraction(tail, denominator) <= ALPHA:
            rank = k
            break
    return {'n': n, 'median': statistics.median(ordered),
            'upper': ordered[rank - 1] if rank else None,
            'lower': ordered[n - rank] if rank else None, 'alpha': float(ALPHA)}


def score(calls, reference, candidate):
    def mean(arm):
        rows = [c['seconds'] for c in calls if c['execution'] == arm]
        require(len(rows) == 2 and all(math.isfinite(x) and x > 0 for x in rows), 'incomplete mirrored block')
        return math.exp(statistics.mean(map(math.log, rows)))
    baseline, changed = mean(reference), mean(candidate)
    return {'ratio': changed / baseline, 'excess_s': changed - baseline - max(.05 * baseline, .001)}


def retained_snapshot(path, raw, columns, meta):
    """Keep compact heads and first-call output arrays, not all timed native dumps."""
    write(path.with_suffix('.json'), raw)
    data = np.memmap(columns, dtype=np.uint64, mode='r', shape=(meta['columns'], meta['rows']))
    try:
        indices = sorted(set(meta['outputs'].values()) | set(meta.get('saved', [])))
        np.savez_compressed(path.with_suffix('.npz'), **{str(i): data[i] for i in indices})
    finally:
        del data
    return {'head': digest(path.with_suffix('.json')), 'outputs': digest(path.with_suffix('.npz'))}


def compare_saved(left, right, meta):
    """Cross-worker/host numerical agreement, using compact first-call outputs."""
    a, b = read(left.with_suffix('.json')), read(right.with_suffix('.json'))
    for matrix, rtol, atol in [('RB', 1e-7, 1e-10), ('RV', 1e-6, 1e-10), ('RFIT', 1e-7, 1e-9)]:
        cmgschur.close(numbers(a['matrices'][matrix]), numbers(b['matrices'][matrix]), matrix, rtol, atol)
    numeric = {meta['outputs'][name] for name in ('residual', 'xb', 'xbd', 'absorbed')} | set(meta.get('saved', []))
    with np.load(left.with_suffix('.npz')) as x, np.load(right.with_suffix('.npz')) as y:
        require(set(x.files) == set(y.files), 'cross-host output channels changed')
        sample = x[str(meta['outputs']['sample'])].view(np.float64) == 1
        for key in x.files:
            xv, yv = x[key].view(np.float64), y[key].view(np.float64)
            require(xv.shape == yv.shape, 'cross-host output shape changed')
            if int(key) not in numeric:
                require(np.array_equal(x[key], y[key]), 'cross-host sample/drop decisions changed')
                continue
            finite = np.isfinite(xv) & (np.abs(xv) < 8e307) & sample
            require(np.array_equal(finite, np.isfinite(yv) & (np.abs(yv) < 8e307) & sample)
                    and np.array_equal(x[key][~finite], y[key][~finite]), 'cross-host missingness changed')
            cmgschur.close(xv[finite], yv[finite], 'cross-host outputs', 1e-7, 1e-9)


def run(manifest_path, output, shard=0, replica=0, resume=False, execute=run_process):
    require(threading.current_thread() is threading.main_thread(), 'run requires the main thread')
    started = time.monotonic()
    manifest = read(manifest_path)
    with Deadline(manifest['budget_s'], started) as deadline:
        return _run(manifest_path, output, manifest, deadline, shard, replica, resume, execute)


def _run(manifest_path, output, manifest, deadline, shard, replica, resume, execute):
    manifest_path, output = Path(manifest_path).resolve(), Path(output).resolve()
    require(manifest['schema'] == 1 and manifest['platform'] == platform.system(), 'wrong manifest/host platform')
    require(0 <= shard < manifest['shards'] and 0 <= replica < manifest['replicas'], 'invalid shard/replica')
    require('continuation' not in manifest or not resume and manifest['continuation']['hostname'] == socket.gethostname(),
            'continuation requires a fresh session on the original host')
    require(output.is_relative_to((ROOT / 'target/tmp').resolve()), 'outputs belong under target/tmp/<task>')
    output.mkdir(parents=True, exist_ok=True)
    directory = output / f'run-{replica}-{shard}'
    path = directory / 'results.json'
    identity = routing.identity(manifest)
    result = {'schema': 1, 'manifest_sha256': identity, 'shard': shard, 'replica': replica,
              'platform': platform.system(), 'hostname': socket.gethostname(), 'status': 'running',
              'cases': [], 'failures': [], 'clock': 'CLOCK_MONOTONIC around complete native call'}
    if path.exists():
        previous = read(path)
        require(resume and previous['manifest_sha256'] == identity and previous['status'] == 'blocked'
                and previous['failures'][-1]['kind'] == 'operational'
                and previous['hostname'] == result['hostname'], 'resume requires unchanged inputs and an operational blocker on this host')
        require(all(c['status'] == 'complete' or not c['blocks'] for c in previous['cases']),
                'partial timing observations require discussion before continuation')
        retained = any(c['status'] == 'complete' for c in previous['cases'])
        require(not retained or manifest['platform'] == 'Darwin' and previous.get('host', {}).get('isolation_ok'),
                'completed cells require valid Mac host evidence; discuss SCC/invalid-host continuation')
        result = previous
        result.setdefault('previous_hosts', []).append(previous.get('host'))
        result['cases'] = [c for c in result['cases'] if c['status'] == 'complete']
        result['status'] = 'running'
    else:
        require(not resume, 'nothing to resume')
        directory.mkdir(exist_ok=False)
    require(len(result['failures']) < 10, 'ten unsuccessful repairs: discuss with Johannes')
    started = deadline.started
    bindings, files = {}, {}
    identities = {}
    host = None
    phase = 'setup'

    def check_files():
        deadline.check()
        require(all(resolve(manifest_path, name) == path for name, path in bindings.items()),
                'an input alias changed during the job')
        require(all(stamp(p) == saved for p, saved in identities.items()), 'an input changed during the job')

    def remaining():
        return min(deadline.check(), manifest['timeout_s'])

    def persist():
        result['elapsed_s'] = time.monotonic() - started
        write(path, result)

    try:
        bindings = continuation_bindings(manifest, manifest_path)
        identities.update({p: stamp(p) for p in bindings.values()})
        validate_continuation(manifest, manifest_path)
        bindings.update({name: resolve(manifest_path, name) for name in manifest['files']})
        files = {bindings[name]: sha for name, sha in manifest['files'].items()}
        identities.update(verify_sources(manifest['execution_sources']))
        result['execution_sources'] = manifest['execution_sources']
        for filename, expected in files.items():
            before = stamp(filename)
            require(digest(filename) == expected and stamp(filename) == before, 'input identity mismatch: ' + str(filename))
            identities[filename] = before
        result['verified_input_files'] = len(files)
        from qualify_scc import HostMonitor, MacMonitor
        monitor = HostMonitor if manifest['platform'] == 'Linux' else MacMonitor
        host = monitor(directory / f"host-{len(result['failures'])}", max(manifest['threads']))
        with deadline.observing(host):
            for index, item in enumerate(manifest['cases']):
                if index % manifest['shards'] != shard:
                    continue
                meta = metadata(manifest, item, manifest_path)
                signatures, first_worker, output_files = {}, {}, []
                for workers in manifest['threads']:
                    cell_id = f"{item['id']}-t{workers}"
                    completed = next((c for c in result['cases'] if c['id'] == cell_id), None)
                    if completed is not None:
                        signatures[workers] = completed['worker_signatures']
                        continue
                    phase = 'setup'
                    cell = {'id': cell_id, 'family': item['family'], 'status': 'running', 'blocks': [], 'warmups': [],
                            'host_attempt': len(result['failures'])}
                    result['cases'].append(cell)
                    work = directory / f"{cell_id}-attempt-{len(result['failures'])}"
                    work.mkdir(exist_ok=False)
                    aliases, prepared, snapshots, requests = {}, {}, {}, {}
                    aliases = artifact_aliases(manifest)
                    for role, artifact in manifest['artifacts'].items():
                        if aliases[role] != role:
                            continue
                        request = work / (role + '.case')
                        source = resolve(manifest_path, item['input'])
                        if artifact['request'] == 'preserve':
                            prepare_case(source, request, workers)
                        else:
                            routing.prepare(source, request, 'baseline' if artifact['request'] == 'auto' else artifact['request'], workers)
                        prepared[role] = request
                        with request.open('rb') as stream:
                            _, arguments = case_header(stream)
                        requests[role] = dict(a.decode().split('=', 1) for a in arguments[1:])
                        identities[request] = stamp(request)
                    cell['aliases'] = aliases
                    persist()

                    def call(role, name, traced):
                        nonlocal phase
                        check_files()
                        if host is not None:
                            host.check()
                        artifact = manifest['artifacts'][role]
                        target, log = work / (name + '.json'), work / (name + '.log')
                        env = ['env', *[v for key in ENVIRONMENT for v in ('-u', key)],
                               *[key + '=1' for key in THREAD_ENV]]
                        if traced:
                            env += ['FEREG_SOLVER_TRACE=1']
                        if item['memory_limit_bytes'] is not None:
                            env += ['FEREG_MEMORY_LIMIT_BYTES=' + str(item['memory_limit_bytes'])]
                        command = [*env, str(resolve(manifest_path, manifest['harness'])),
                                   str(resolve(manifest_path, artifact['path'])), '--replay', str(prepared[role]), str(target)]
                        timeout = remaining()
                        phase = 'native'
                        native_started = time.monotonic_ns()
                        resources = execute(command, log, timeout)
                        native_finished = time.monotonic_ns()
                        deadline.check()
                        phase = 'science'
                        raw, columns, detail = read_native(target, log, meta, role not in snapshots)
                        requested = requests[role]['solver']
                        expected = 8 if requested == 'cmgschur' else cmgschur.EXPECTED_SOLVER[requested]
                        require(detail['solver_rows'][0][0] == expected, 'requested solver identity mismatch')
                        if role in snapshots:
                            require(detail['fingerprint'] == snapshots[role][2]['fingerprint'], 'fixed-worker repeat changed')
                        if traced and artifact['families']:
                            eligible = affected(meta, artifact['families'])
                            if eligible:
                                family = next(f for f in artifact['families'] if affected(meta, [f]))
                                detail.update(science.resource_diagnostics(log.read_text(), 'incremental-' + family,
                                              False, {}, expected_preset='routing_incremental_' + family))
                        detail.update(execution=role, maxrss_bytes=resources['maxrss_bytes'],
                                      started_monotonic_ns=native_started, finished_monotonic_ns=native_finished)
                        phase = 'setup'
                        return raw, columns, detail

                    for role in prepared:
                        snapshot = call(role, 'warmup-' + role, True)
                        snapshots[role] = snapshot
                        cell['warmups'].append(snapshot[2])
                    conceptual = {role: snapshots[target] for role, target in aliases.items()}
                    phase = 'science'
                    compare_artifacts(manifest, meta, conceptual)
                    signatures[workers] = {role: worker_signature(s[0])
                                           for role, s in conceptual.items()}
                    require(all(v == signatures[workers] for v in signatures.values()), 'cross-worker discrete decisions changed')
                    if first_worker:
                        for role, current in conceptual.items():
                            prior = first_worker[role]
                            cmgschur.scientific_comparison(prior[0], current[0], prior[1], current[1], meta)
                    first_worker = first_worker or conceptual
                    cell['worker_signatures'] = signatures[workers]
                    saved = {role: retained_snapshot(work / ('reference-' + role), s[0], s[1], meta)
                             for role, s in snapshots.items()}
                    cell['references'] = {role: str((work / ('reference-' + alias)).relative_to(directory))
                                          for role, alias in aliases.items()}
                    cell['reference_files'] = {role: saved[alias] for role, alias in aliases.items()}
                    for block, order in enumerate(orders(list(prepared), manifest['blocks'], manifest['seed'] + index + replica * 1009)):
                        if block not in block_indices(manifest, cell_id):
                            continue
                        phase = 'setup'
                        row = {'index': block, 'order': order, 'calls': [], 'status': 'running'}
                        cell['blocks'].append(row)
                        persist()
                        for number, role in enumerate(order):
                            raw, columns, detail = call(role, f'block-{block}-{number}', False)
                            row['calls'].append(detail)
                            columns.unlink()
                            columns.with_suffix('').unlink()
                            (work / f'block-{block}-{number}.log').unlink()
                        row['comparisons'] = {role: score(row['calls'], aliases[role], aliases['candidate'])
                                              for role in ('original', 'incumbent', 'hidden') if role in aliases}
                        row['status'] = 'complete'
                        persist()
                    cell['status'] = 'complete'
                    for request in prepared.values():
                        identities.pop(request)
                        request.unlink()
                    output_files.extend(snapshot[1] for snapshot in snapshots.values())
                    persist()
                for columns in output_files:
                    columns.unlink()
            phase = 'setup'
            check_files()
        if host is not None:
            result['host'] = host.report()
            require(result['host']['isolation_ok'], 'host observation did not complete')
        check_files()
        result['status'] = 'complete'
        result['budget_compliant'] = True
        persist()
        check_files()  # Includes the final output write, even if a C call delayed the alarm.
    except BaseException as error:
        deadline.cleanup()
        result['status'] = 'blocked'
        expired = deadline.expired or time.monotonic() - started >= manifest['budget_s']
        result['budget_compliant'] = not expired
        result['failures'].append({'kind': 'budget' if expired else 'scientific' if phase == 'science' else
                                  'native' if phase == 'native' else 'operational',
                                  'error': repr(error), 'time': time.time()})
        result['next_action'] = 'Diagnose and repair; discuss unresolved failure with Johannes. Keep candidate and defaults; do not advance.'
        if host is not None:
            result['host'] = host.report()
        result['cleanup_overrun_s'] = max(0, time.monotonic() - started - manifest['budget_s'])
        persist()
    return result


def accounting(text, host):
    values = {}
    for line in text.splitlines():
        pair = line.split(None, 1)
        if len(pair) == 2:
            require(pair[0] not in values, 'accounting must contain one task')
            values[pair[0]] = pair[1].strip()
    require(values.get('jobnumber') == str(host['job_id']) and values.get('taskid') == str(host['task_id'])
            and values.get('hostname', '').split('.')[0] == host['hostname'].split('.')[0]
            and values.get('project') == 'welfgr' and values.get('failed', '').split()[0:1] == ['0']
            and values.get('exit_status') == '0' and values.get('end_time'), 'missing/failed SGE accounting')


def report(manifest_path, output, accounting_dir=None):
    manifest, output = read(manifest_path), Path(output)
    results, errors, comparisons, workers, snapshots, metadata_cache = [], [], {}, {}, {}, {}
    source_stamps = {}
    prior_bindings = {}
    try:
        prior_bindings = continuation_bindings(manifest, manifest_path)
        source_stamps.update({p: stamp(p) for p in prior_bindings.values()})
        validate_continuation(manifest, manifest_path)
        source_stamps.update(verify_sources(manifest['execution_sources']))
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append('execution sources: ' + str(error))
    aliases = artifact_aliases(manifest)
    cases = {item['id']: (index, item) for index, item in enumerate(manifest['cases'])}
    for replica in range(manifest['replicas']):
        for shard in range(manifest['shards']):
            path = output / f'run-{replica}-{shard}/results.json'
            try:
                result = read(path)
                require(result['manifest_sha256'] == routing.identity(manifest)
                        and (result['replica'], result['shard']) == (replica, shard)
                        and result['status'] == 'complete', 'missing, changed or failed shard')
                require(result.get('execution_sources') == manifest['execution_sources'], 'unverified execution sources')
                require('continuation' not in manifest or result['hostname'] == manifest['continuation']['hostname'],
                        'continuation host changed')
                require(result.get('budget_compliant') is True
                        and 0 <= result['elapsed_s'] < manifest['budget_s'], 'unverified run budget')
                require(result['host']['isolation_ok'], 'missing host observation')
                if manifest['platform'] == 'Linux':
                    host = result['host']['host']
                    require(result['hostname'].split('.')[0] == host['hostname'].split('.')[0],
                            'recorded host differs from accounted host')
                    require(accounting_dir is not None, 'SGE accounting is required')
                    accounting((Path(accounting_dir) / f"{host['job_id']}.{host['task_id']}.txt").read_text(), host)
                expected = {f"{c['id']}-t{t}" for i, c in enumerate(manifest['cases'])
                            if i % manifest['shards'] == shard for t in manifest['threads']}
                require(len(result['cases']) == len(expected) and {c['id'] for c in result['cases']} == expected,
                        'incomplete or duplicate workload coverage')
                last_finished = 0
                for cell in result['cases']:
                    indices = list(block_indices(manifest, cell['id']))
                    require(cell['status'] == 'complete' and [r['index'] for r in cell['blocks']] == indices, 'incomplete cell')
                    hosts = [*result.get('previous_hosts', []), result['host']]
                    require(0 <= cell['host_attempt'] < len(hosts) and hosts[cell['host_attempt']]
                            and hosts[cell['host_attempt']]['isolation_ok'], 'reused cell lacks valid host evidence')
                    key = cell['id'].rsplit('-t', 1)[0]
                    index, item = cases[key]
                    require(cell['aliases'] == aliases, 'artifact roles changed')
                    require(set(cell['worker_signatures']) == set(cell['references']) == set(cell['reference_files']) == set(aliases)
                            and all(routing.sha(s) for s in cell['worker_signatures'].values()),
                            'missing scientific reference or worker signature')
                    warmups = {c['execution']: c['fingerprint'] for c in cell['warmups']}
                    require(len(cell['warmups']) == len(set(aliases.values())) and set(warmups) == set(aliases.values()),
                            'missing/duplicate warmup')
                    for call in [*cell['warmups'], *(c for row in cell['blocks'] for c in row['calls'])]:
                        start, finish = call['started_monotonic_ns'], call['finished_monotonic_ns']
                        require(type(start) is type(finish) is int and 0 < start and last_finished <= start <= finish,
                                'native execution intervals overlap or are invalid')
                        last_finished = finish
                    expected_orders = orders(list(dict.fromkeys(aliases.values())), manifest['blocks'],
                                             manifest['seed'] + index + replica * 1009)
                    require(key not in workers or workers[key] == cell['worker_signatures'], 'cross-host/worker discrete mismatch')
                    workers[key] = cell['worker_signatures']
                    for role, relative in cell['references'].items():
                        current = path.parent / relative
                        require(current.resolve().is_relative_to(path.parent.resolve()), 'reference outside job directory')
                        for suffix, name in (('.json', 'head'), ('.npz', 'outputs')):
                            require(digest(current.with_suffix(suffix)) == cell['reference_files'][role][name],
                                    'retained scientific output changed')
                        validate_result(read(current.with_suffix('.json')))
                        if (key, role) in snapshots:
                            if key not in metadata_cache:
                                metadata_cache[key] = routing_cases.inspect_case(resolve(manifest_path, item['input']), item['family'])
                            meta = metadata_cache[key]
                            compare_saved(snapshots[key, role], current, meta)
                        snapshots[key, role] = current
                    for block, row in zip(indices, cell['blocks']):
                        require(row['status'] == 'complete' and row['index'] == block
                                and [c['execution'] for c in row['calls']] == row['order'] == expected_orders[block],
                                'incomplete/reordered block')
                        require(all(c['fingerprint'] == warmups[c['execution']] for c in row['calls']),
                                'timed repeat lost independent scientific agreement')
                        for role in ('original', 'incumbent', 'hidden'):
                            if role not in cell['aliases']:
                                continue
                            value = score(row['calls'], cell['aliases'][role], cell['aliases']['candidate'])
                            comparisons.setdefault((cell['id'], role), []).append((result['hostname'].split('.')[0], value))
                results.append(result)
            except (OSError, ValueError, KeyError, TypeError) as error:
                errors.append(f'replica {replica}, shard {shard}: {error}')
    rows = []
    for (cell, role), values in sorted(comparisons.items()):
        if manifest['platform'] == 'Linux':
            # Repeated solves/allocations on one physical host are one unit.
            hosts = sorted({host for host, _ in values})
            scores = [statistics.median(v['excess_s'] for h, v in values if h == host) for host in hosts]
        else:
            scores = [v['excess_s'] for _, v in values]
        bound = interval(scores)
        state = 'passed' if bound['upper'] is not None and bound['upper'] <= 0 else (
            'regression' if bound['lower'] is not None and bound['lower'] > 0 else 'inconclusive')
        rows.append({'case': cell, 'reference': role, 'status': state, 'excess_seconds': bound,
                     'median_ratio': statistics.median(v['ratio'] for _, v in values)})
    try:
        require(all(stamp(path) == saved for path, saved in source_stamps.items()),
                'execution sources changed during report')
        require(all(resolve(manifest_path, name) == p for name, p in prior_bindings.items()), 'prior evidence alias changed')
    except (OSError, ValueError) as error:
        errors.append(str(error))
    complete = not errors and bool(rows)
    passed = complete and all(row['status'] == 'passed' for row in rows)
    summary = {'schema': 1, 'manifest_sha256': routing.identity(manifest), 'platform': manifest['platform'],
               'status': 'smoke_complete' if complete and manifest['coverage'] == 'smoke' else
                         'passed' if passed else 'needs_discussion',
               'errors': errors, 'comparisons': rows, 'native_calls': sum(
                   len(c['warmups']) + sum(len(b['calls']) for b in c['blocks']) for r in results for c in r['cases']),
               'analysis_unit': 'physical host' if manifest['platform'] == 'Linux' else 'mirrored block',
               'coverage': 'One-sided .025 bounds conditional on independent unit scores with a common median; no simultaneous interval claim.',
               'next_action': 'Review platform-specific evidence; no automatic activation or successor.' if passed else
                              'Discuss failed, missing or inconclusive evidence with Johannes; keep candidate and defaults.'}
    if 'continuation' in manifest:
        state = manifest['continuation']
        summary['continuation'] = {k: state[k] for k in ('prior_plan', 'prior_result', 'prior_calls', 'prior_elapsed_s',
                                  'prior_complete_blocks', 'prior_partial_blocks', 'warmup_calls', 'timed_calls', 'original_call_cap')}
        summary['continuation'].update(prior_status='blocked', prior_observations_used=False,
            planned_new_calls=state['warmup_calls']+state['timed_calls'],
            planned_total_calls=state['prior_calls']+state['warmup_calls']+state['timed_calls'],
            cumulative_elapsed_s=state['prior_elapsed_s']+results[0]['elapsed_s'] if len(results) == 1 else None)
    write(output / 'summary.json', summary)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='action', required=True)
    prepare = commands.add_parser('plan')
    prepare.add_argument('config', type=Path)
    prepare.add_argument('--output', type=Path, required=True)
    continuation = commands.add_parser('continue-plan')
    continuation.add_argument('manifest', type=Path)
    continuation.add_argument('results', type=Path)
    continuation.add_argument('--output', type=Path, required=True)
    for name in ('run', 'report'):
        command = commands.add_parser(name)
        command.add_argument('manifest', type=Path)
        command.add_argument('--output', type=Path, required=True)
        if name == 'run':
            command.add_argument('--shard', type=int, default=0)
            command.add_argument('--replica', type=int, default=0)
            command.add_argument('--resume', action='store_true')
        else:
            command.add_argument('--accounting', type=Path)
    args = parser.parse_args()
    if args.action == 'continue-plan':
        continuation_plan(args.manifest, args.results, args.output)
        print('Planned untouched Mac blocks within the original budgets; no native calls.')
        return 0
    if args.action == 'plan':
        result = plan(args.config, args.output)
        print(f"Planned {len(result['cases']) * len(result['threads'])} cells; no native calls.")
        return 0
    if args.action == 'run':
        def interrupted(*_):
            raise KeyboardInterrupt('qualification interrupted')
        signal.signal(signal.SIGTERM, interrupted)
        result = run(args.manifest, args.output, args.shard, args.replica, args.resume)
        print(result['status'])
        return 0 if result['status'] == 'complete' else 1
    result = report(args.manifest, args.output, args.accounting)
    print(result['status'])
    return 0 if result['status'] in {'passed', 'smoke_complete'} else 1


if __name__ == '__main__':
    raise SystemExit(main())
