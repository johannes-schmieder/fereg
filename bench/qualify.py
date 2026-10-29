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
import qualify_ownership as ownership
from native import run_process
from native_io import ROOT, case_header, diagnostics, digest, numbers, prepare_case, validate_result
from replay_plugins import exact_view

FAMILIES = {'no-fe', 'one-fe', 'slopes', 'low-saved', *routing_cases.NATIVE_CASES,
            *routing_cases.SYNTHETIC_CASES, *routing_cases.EXTENDED_CASES,
            *routing_cases.SAMPLED_CASES}
CAMPAIGN_FAMILIES = set(routing_cases.CAMPAIGN_CASES)
# Qualification coverages and their complete family sets, at workers [1, 4].
COVERAGES = {'routing': FAMILIES, 'campaign': CAMPAIGN_FAMILIES,
             'routing-campaign': FAMILIES | CAMPAIGN_FAMILIES}
# Public two-FE Auto policies an original/incumbent artifact may declare it runs;
# omission keeps the meaning of configurations written before beta.8.
PUBLIC_POLICIES = (routing.GATES, routing.GATES_V2)
ENVIRONMENT = ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING',
               'FEREG_CMG_ORDINARY', 'FEREG_ACTION_MEMORY_AUDIT', 'FEREG_PPML_MEMORY_TRACE',
               'FEREG_MEMORY_LIMIT_BYTES', 'LLVM_PROFILE_FILE')
THREAD_ENV = ('OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS', 'MKL_NUM_THREADS',
              'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS')
ALPHA = Fraction(1, 40)
SOURCE_MODULES = ('qualify_scc', 'block_host', 'native', 'native_io', 'routing',
                  'routing_cases', 'routing_oracle', 'routing_science', 'cmgschur',
                  'cmgschur_cases', 'time_exact', 'replay_plugins', 'replay_psd',
                  'replay_bias_reference', 'replay_cmg_polish', 'replay_ols_reference', 'cmg_setup',
                  'two_fe_policy', 'two_fe_policy_development', 'qualify_ownership', 'ownership_qualification_cases', 'ownership_panel3')


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


def diagnostic_mode(manifest):
    if 'diagnostic' not in manifest:
        return False
    diagnostic_tool(manifest).validate(manifest)
    return True


def diagnostic_tool(manifest):
    import two_fe_policy
    import two_fe_policy_development
    import cmg_setup
    if manifest.get('diagnostic') == two_fe_policy_development.MODE:
        return two_fe_policy_development
    return two_fe_policy if manifest.get('diagnostic') == two_fe_policy.MODE else cmg_setup


def comparison_pairs(manifest):
    if diagnostic_mode(manifest):
        return diagnostic_tool(manifest).comparisons(manifest)
    return tuple((role, 'candidate') for role in ('original', 'incumbent', 'hidden')
                 if role in manifest['artifacts'])


def comparison_key(manifest, reference, target):
    return (reference + '->' + target if manifest.get('diagnostic') == 'two-fe-remaining-calibration-v1'
            else reference)


def minimum_hosts_per_cell(manifest):
    """Prospective Linux gate; omission preserves historical manifests."""
    if 'min_distinct_hosts_per_cell' not in manifest:
        return None
    minimum = manifest['min_distinct_hosts_per_cell']
    require(manifest['platform'] == 'Linux', 'min_distinct_hosts_per_cell requires Linux')
    require(type(minimum) is int and type(manifest['replicas']) is int
            and 1 <= minimum <= manifest['replicas'] <= 12,
            'min_distinct_hosts_per_cell must be an integer in 1..replicas (at most 12)')
    return minimum


def physical_host(hostname, normalize=False):
    host = hostname.split('.')[0]
    if normalize:
        require(bool(host) and hostname == hostname.strip(), 'invalid physical hostname')
        return host.lower()
    return host


def plan(config_path, output, *, configuration=None):
    """Hash each actual input once. No native call, oracle solve, or history walk."""
    config_path, output = Path(config_path).resolve(), Path(output).resolve()
    require(not output.exists(), 'plan already exists')
    value = read(config_path) if configuration is None else copy.deepcopy(configuration)
    diagnostic = diagnostic_mode(value)
    ownership_mode = ownership.enabled(value)
    within_migration = value.get('scope') == science.WITHIN_MIGRATION
    if ownership_mode:
        ownership.validate(value)
    required = {'scope', 'platform', 'artifacts', 'harness', 'cases', 'threads',
                'blocks', 'shards', 'replicas', 'seed', 'timeout_s', 'budget_s', 'coverage'}
    if ownership_mode:
        required.update(('qualification', 'ownership_binding'))
    if diagnostic:
        required.add('diagnostic')
    if value.get('diagnostic') == 'two-fe-gates-development-v1':
        required.add('policy_binding')
    if 'min_distinct_hosts_per_cell' in value:
        required.add('min_distinct_hosts_per_cell')
    require(set(value) == required, 'configuration needs exactly: ' + ', '.join(sorted(required)))
    require(value['scope'] in {'exact', 'many', 'two', science.WITHIN_MIGRATION}, 'unsupported numerical scope')
    require(not within_migration or not diagnostic and not ownership_mode,
            'within migration cannot change routing or ownership policies')
    require(value['platform'] in {'Darwin', 'Linux'}, 'platform must be Darwin or Linux')
    require(value['coverage'] in ({'ownership'} if ownership_mode else {*COVERAGES, 'smoke'}), 'unsupported coverage')
    for field, low, high in [('blocks', 1, 24), ('shards', 1, 42), ('replicas', 1, 12),
                             ('seed', 0, 2**64 - 1), ('timeout_s', 1, 120), ('budget_s', 1, 14400)]:
        require(type(value[field]) is int and low <= value[field] <= high, 'invalid ' + field)
    minimum_hosts_per_cell(value)
    workers = value['threads']
    require(isinstance(workers, list) and workers and len(set(workers)) == len(workers)
            and all(type(n) is int and 1 <= n <= 1024 for n in workers), 'invalid workers')
    require(value['platform'] != 'Darwin' or value['replicas'] == value['shards'] == 1,
            'Mac timing uses one serial host run')
    artifacts = value['artifacts']
    require(diagnostic or set(artifacts) in ({'original', 'incumbent', 'candidate'},
                              {'original', 'incumbent', 'candidate', 'hidden'}), 'required artifact roles missing')
    files = {}

    def bind(name):
        path = (config_path.parent / name).resolve(strict=True)
        key = os.path.relpath(path, output.parent)
        if key not in files:
            files[key] = digest(path)
        return key

    for role, artifact in artifacts.items():
        require(set(artifact) - {'public_policy'} == {'path', 'revision', 'build', 'request', 'families'},
                'invalid artifact ' + role)
        require('public_policy' not in artifact or artifact['public_policy'] in PUBLIC_POLICIES
                and artifact['request'] == 'auto' and not ownership_mode, 'invalid public two-FE policy for ' + role)
        require(isinstance(artifact['revision'], str) and artifact['revision'], 'record artifact revision')
        require(isinstance(artifact['build'], str) and artifact['build'], 'record compiler and build settings')
        require(artifact['request'] in ({'auto', *ownership.REQUESTS} if ownership_mode else
                {'auto', 'preserve', *routing.INCREMENTAL, *routing.REMAINING, *routing.GATE_RULES,
                 routing.GEOMETRY_MANY}), 'unsupported request')
        require(artifact['request'] not in routing.REMAINING[1:] or
                value.get('diagnostic') == 'two-fe-remaining-calibration-v1',
                'forced/never controls belong only to the frozen calibration')
        families = {science.WITHIN_MIGRATION} if within_migration else {'many', 'two'}
        require(isinstance(artifact['families'], list) and set(artifact['families']) <= families,
                'invalid artifact numerical families')
        artifact['path'] = bind(artifact['path'])
    if value.get('diagnostic') == 'two-fe-gates-development-v1':
        for name in ('fit', 'spec'):
            value['policy_binding'][name] = bind(value['policy_binding'][name])
    if ownership_mode:
        value['ownership_binding'] = {k: bind(v) for k, v in value['ownership_binding'].items()}
    if within_migration:
        require(set(artifacts) == {'original', 'incumbent', 'candidate'}
                and all(a['request'] == 'auto' for a in artifacts.values())
                and len({a.get('public_policy', routing.GATES) for a in artifacts.values()}) == 1,
                'within migration compares unchanged public Auto requests')
        require(artifacts['original']['families'] == artifacts['incumbent']['families'] == []
                and artifacts['candidate']['families'] == [science.WITHIN_MIGRATION],
                'within migration must declare only its dependency family')
    elif not diagnostic and not ownership_mode:
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
            allowed_hidden = {'incremental-' + value['scope']}
            if value['scope'] == 'two':
                allowed_hidden.add(routing.REMAINING[0])
                allowed_hidden.update(routing.GATE_RULES)
            if value['scope'] == 'many':
                allowed_hidden.add(routing.GEOMETRY_MANY)
            require('hidden' in artifacts and artifacts['hidden']['request'] in allowed_hidden,
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
        if diagnostic:
            diagnostic_tool(value).validate_case(source, item, mode=value['diagnostic'])
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
    if value['coverage'] in COVERAGES:
        expected = COVERAGES[value['coverage']]
        require(len(value['cases']) == len(expected) and {c['family'] for c in value['cases']} == expected
                and sorted(workers) == [1, 4],
                f"{value['coverage']} qualification requires all {2 * len(expected)} workload cells")
        require(value['blocks'] % 2 == 0 and value['blocks'] >= (6 if value['platform'] == 'Darwin' else 2),
                'qualification requires balanced blocks and enough independent measurements')
    if value.get('diagnostic') in ('two-fe-remaining-calibration-v1', 'two-fe-gates-development-v1'):
        diagnostic_tool(value).validate_panel(value, output)
    value.update(schema=1, files=files, warmups=1,
                 execution_sources={name: digest(path) for name, path in execution_sources().items()})
    if diagnostic:
        diagnostic_mode(value)
    if ownership_mode:
        ownership.validate_inputs(value, output)
    if value['platform'] == 'Darwin':
        value['mac_cell_policy'] = fresh_mac_policy(value)
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
        require(item['family'] == 'inactive-columns' or ownership.enabled(manifest)
                and item['family'] == 'ownership-inactive-zero', 'inactive oracle has wrong family')
        meta['oracle'] = routing_cases.inactive_case_oracle(source)
    require(any(key in meta for key in ('routing_oracle', 'oracle', 'replay_reference')), 'missing independent truth')
    return meta


def continuation_accounting(old, previous):
    """Debit started slots, never select or reuse previous timing observations."""
    require(not any(k in old for k in ('continuation', 'recovery', 'mac_cell_policy')) and old['platform'] == 'Darwin'
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
    if old['coverage'] in COVERAGES:
        expected = COVERAGES[old['coverage']]
        require(len(cells) == 2 * len(expected) and {c['family'] for c in old['cases']} == expected
                and sorted(old['threads']) == [1, 4], f'continuation requires all {2 * len(expected)} cells')
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
    require(not ownership.enabled(old), 'ownership qualification cannot add a continuation budget')
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
    if 'recovery' in manifest:
        return manifest['recovery']['block_indices'][cell_id]
    return manifest['continuation']['block_indices'][cell_id] if 'continuation' in manifest else range(manifest['blocks'])


def cell_policy(manifest):
    return manifest.get('recovery', manifest.get('mac_cell_policy'))


def fresh_mac_policy(manifest):
    """Fixed execution bounds, independent of any prior evidence or provenance."""
    from block_host import MAC_RECOVERY_POLICY
    cells = [f"{item['id']}-t{t}" for item in manifest['cases'] for t in manifest['threads']]
    per_cell = len(set(artifact_aliases(manifest).values()))*(1+2*manifest['blocks'])
    process = copy.deepcopy(MAC_RECOVERY_POLICY)
    process['executables'] += ['actual_plugin_loader_test', 'actual_plugin_l']
    retries = 0 if diagnostic_mode(manifest) or ownership.enabled(manifest) else 4
    return {'fresh_cells': cells, 'max_cell_retries': int(bool(retries)), 'max_retry_cells': retries,
            'quiet_timeout_s': 120, 'calls_per_cell': per_cell,
            'baseline_calls': len(cells)*per_cell,
            'new_call_cap': (len(cells)+min(retries, len(cells)))*per_cell,
            'process_policy': process}


def validate_cell_policy(manifest):
    if diagnostic_mode(manifest):
        require(manifest.get('schema') == manifest.get('warmups') == 1
                and (manifest['platform'] != 'Darwin' or 'mac_cell_policy' in manifest),
                'diagnostic requires its frozen warmup and Mac observation policy')
    require(sum(k in manifest for k in ('continuation', 'recovery', 'mac_cell_policy')) <= 1,
            'Mac execution policies are mutually exclusive')
    if 'mac_cell_policy' in manifest:
        require(manifest['platform'] == 'Darwin' and manifest['shards'] == manifest['replicas'] == 1
                and type(manifest['budget_s']) is int and 1 <= manifest['budget_s'] <= 14400
                and type(manifest['timeout_s']) is int and 1 <= manifest['timeout_s'] <= 120
                and manifest['mac_cell_policy'] == fresh_mac_policy(manifest), 'invalid fresh Mac cell policy')


def cell_calls(cell):
    return [*cell['warmups'], *(call for row in cell['blocks'] for call in row['calls'])]


def cell_records(cell, manifest, index, indices):
    """Validate one complete cell without considering its performance scores."""
    aliases = artifact_aliases(manifest)
    arms = list(dict.fromkeys(aliases.values()))
    require(cell['status'] == 'complete' and cell['aliases'] == aliases
            and [r['index'] for r in cell['blocks']] == list(indices), 'incomplete recovery cell')
    require([c['execution'] for c in cell['warmups']] == arms, 'invalid recovery warmups')
    warm = {c['execution']: c['fingerprint'] for c in cell['warmups']}
    expected = orders(arms, manifest['blocks'], manifest['seed']+index)
    for row in cell['blocks']:
        require(row['status'] == 'complete' and row['order'] == expected[row['index']]
                and [c['execution'] for c in row['calls']] == row['order']
                and all(c['fingerprint'] == warm[c['execution']] for c in row['calls']), 'invalid recovery repeats/order')
    calls, end = cell_calls(cell), 0
    for call in calls:
        start, finish = call['started_monotonic_ns'], call['finished_monotonic_ns']
        require(type(start) is type(finish) is int and 0 < start and end <= start <= finish
                and math.isfinite(call['seconds']) and call['seconds'] > 0, 'invalid recovery call interval')
        end = finish
    return calls[0]['started_monotonic_ns'], end


def span_evidence(events, start, end):
    """Require complete sampled coverage and return only externally recorded hits."""
    samples = [row for row in events if row['kind'] == 'process']
    require(samples and all(a['monotonic_ns'] < b['monotonic_ns'] for a, b in zip(samples, samples[1:])),
            'unordered/missing process samples')
    before = [i for i, row in enumerate(samples) if row['monotonic_ns'] <= start]
    after = [i for i, row in enumerate(samples) if row['monotonic_ns'] >= end]
    require(before and after, 'cell span lacks sealed host evidence')
    selected = samples[before[-1]:after[0]+1]
    require(all(0 <= r['finished_monotonic_ns']-r['monotonic_ns'] <= 7.5e9 for r in selected)
            and all(b['monotonic_ns']-a['monotonic_ns'] <= 7.5e9 for a, b in zip(selected, selected[1:])),
            'incomplete host sampling within cell span')
    require(not any(r['kind'] == 'error' and start <= r['monotonic_ns'] <= end
                    and r['error'] != 'known competing compute observed' for r in events), 'observer fault within cell span')
    hits = [h for r in selected for h in r['competing_compute']
            if h['start_monotonic_ns'] <= end and h['end_monotonic_ns'] >= start]
    return {'start_monotonic_ns': start, 'end_monotonic_ns': end, 'samples': len(selected), 'interference': hits}


def recovery_definition(old, previous, events, paths, output):
    require(not ownership.enabled(old), 'ownership qualification cannot add recovery calls')
    require(not any(k in old for k in ('recovery', 'mac_cell_policy')) and old['platform'] == previous['platform'] == 'Darwin'
            and old['shards'] == old['replicas'] == 1 and previous['shard'] == previous['replica'] == 0,
            'recovery requires a serial Mac round')
    require(previous['manifest_sha256'] == routing.identity(old) and previous['status'] == 'blocked'
            and previous['execution_sources'] == old['execution_sources'] and not previous.get('previous_hosts')
            and previous['failures'] and all(f['kind'] == 'operational' for f in previous['failures'])
            and previous['budget_compliant'] is True and previous['cleanup_overrun_s'] == 0,
            'recovery requires unchanged, operationally blocked evidence')
    require(previous['hostname'] == previous['host']['host']['hostname']
            and previous['host']['host']['platform'] == 'Darwin', 'recovery host mismatch')
    expected = [(f"{item['id']}-t{t}", i) for i, item in enumerate(old['cases']) for t in old['threads']]
    require([c['id'] for c in previous['cases']] == [cid for cid, _ in expected[:len(previous['cases'])]],
            'prior cells missing, duplicated or reordered')
    reused, evidence, indices, files = [], {}, {}, dict(old['files'])
    prior = {c['id']: c for c in previous['cases']}
    for cid, index in expected:
        cell = prior.get(cid)
        keep = False
        if cell and cell['status'] == 'complete':
            start, end = cell_records(cell, old, index, block_indices(old, cid))
            evidence[cid] = span_evidence(events, start, end)
            keep = not evidence[cid]['interference']
        if keep:
            reused.append(cid)
            indices[cid] = list(block_indices(old, cid))
            require(set(cell['references']) == set(cell['reference_files']) == set(cell['worker_signatures'])
                    == set(artifact_aliases(old)), 'missing retained science')
            for role, relative in cell['references'].items():
                base = (paths['prior_result'].parent / relative).resolve()
                require(base.is_relative_to(paths['prior_result'].parent), 'retained reference outside prior job')
                for suffix, key in (('.json', 'head'), ('.npz', 'outputs')):
                    files[os.path.relpath(base.with_suffix(suffix), output.parent)] = cell['reference_files'][role][key]
        else:
            indices[cid] = list(range(old['blocks']))
    fresh = [cid for cid, _ in expected if cid not in reused]
    require(fresh and len(expected) == len(set(cid for cid, _ in expected)), 'no fresh cells or duplicate cells')
    if old['coverage'] in COVERAGES:
        families = COVERAGES[old['coverage']]
        require(len(expected) == 2 * len(families) and {c['family'] for c in old['cases']} == families
                and sorted(old['threads']) == [1, 4], f'recovery requires all {2 * len(families)} cells')
    per_cell = len(set(artifact_aliases(old).values()))*(1+2*old['blocks'])
    spent = old.get('continuation', {}).get('prior_calls', 0)+sum(len(cell_calls(c)) for c in previous['cases'])
    elapsed = old.get('continuation', {}).get('prior_elapsed_s', 0)+previous['elapsed_s']
    budget = math.floor(old['budget_s']-previous['elapsed_s'])
    require(0 < budget <= 14400 and 0 <= previous['elapsed_s'] < old['budget_s'], 'recovery budget exhausted')
    refs = {key: {'path': os.path.relpath(p, output.parent), 'sha256': digest(p)} for key, p in paths.items()}
    review = read(paths['prior_review'])
    require(review['kind'] == 'compact_mac_remainder_independent_failure_review'
            and review['status'] == 'passed_failure_review' and review['completed_cells'] >= len(reused)
            and isinstance(review['limits'], list) and review['limits'], 'missing retained independent science review')
    for role, key in (('plan', 'prior_plan'), ('results', 'prior_result'), ('observer', 'prior_host')):
        require((ROOT / review[role]['path']).resolve(strict=True) == paths[key]
                and review[role]['sha256'] == refs[key]['sha256'], 'scientific review evidence differs')
    files.update({r['path']: r['sha256'] for r in refs.values()})
    state = {**refs, 'hostname': previous['hostname'], 'prior_execution_sources': old['execution_sources'],
             'reused_cells': reused, 'fresh_cells': fresh, 'block_indices': indices, 'retained_spans': evidence,
             'prior_calls': spent, 'prior_elapsed_s': elapsed, 'max_cell_retries': 1, 'max_retry_cells': 4,
             'calls_per_cell': per_cell, 'baseline_calls': len(fresh)*per_cell,
             'new_call_cap': (len(fresh)+min(4, len(fresh)))*per_cell, 'quiet_timeout_s': 120}
    state['scientific_review_limits'] = review['limits']
    from block_host import MAC_RECOVERY_POLICY
    state['process_policy'] = copy.deepcopy(MAC_RECOVERY_POLICY)
    state['process_policy']['executables'] += ['actual_plugin_loader_test', 'actual_plugin_l']
    value = copy.deepcopy(old)
    value.pop('continuation', None)
    value.update(recovery=state, budget_s=budget, files=files)
    return value


def recovery_plan(old_path, result_path, host_path, review_path, output, publish=True):
    output = Path(output).resolve()
    paths = dict(zip(('prior_plan', 'prior_result', 'prior_host', 'prior_review'), map(lambda p: Path(p).resolve(strict=True),
                                                                                 (old_path, result_path, host_path, review_path))))
    require(output.parent == paths['prior_plan'].parent and (not publish or not output.exists()), 'invalid recovery plan location')
    old, previous = read(paths['prior_plan']), read(paths['prior_result'])
    require(paths['prior_host'] == paths['prior_result'].parent / 'host-0' / previous['host']['observations'],
            'raw host evidence is not from the prior job')
    events = [json.loads(line) for line in paths['prior_host'].read_text().splitlines()]
    value = recovery_definition(old, previous, events, paths, output)
    if publish:
        require(value['recovery']['hostname'] == socket.gethostname(), 'recovery requires the original host')
        for relative, expected in value['files'].items():
            p = resolve(output, relative)
            before = stamp(p)
            require(digest(p) == expected and stamp(p) == before, 'recovery file identity mismatch: '+relative)
        value['execution_sources'] = {name: digest(p) for name, p in execution_sources().items()}
        write(output, value)
    return value


def validate_recovery(manifest, path):
    if 'recovery' in manifest:
        state = manifest['recovery']
        expected = recovery_plan(*(resolve(path, state[k]['path']) for k in ('prior_plan', 'prior_result', 'prior_host', 'prior_review')),
                                 path, publish=False)
        expected['execution_sources'] = manifest['execution_sources']
        require(manifest == expected, 'recovery selection, bindings or budgets changed')


def recovery_prior(manifest, path):
    state = manifest['recovery']
    prior_path = resolve(path, state['prior_result']['path'])
    return {c['id']: c for c in read(prior_path)['cases']}, prior_path.parent


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
    return raw, columns, {**detail, 'seconds': float(match[0]), 'timing_field': 'NATIVE_SECONDS',
                          'fingerprint': routing.identity(signature)}


def worker_signature(raw):
    solver = numbers(raw['matrices']['RSOLVER'])
    return routing.identity({'model': science.model_discrete_sha256(raw),
        'solver': [[solver[16 * row + i] for i in (0, 1, 3, 10, 11, 12, 13, 14)] for row in range(2)]})


def affected(meta, families):
    return any(science.within_migration_eligible(meta) if family == science.WITHIN_MIGRATION
               else routing.incremental_eligible(meta, family) for family in families)


def resource_policy(manifest, role, meta):
    """Resolve the executed family, including a public candidate's hidden binding."""
    if manifest.get('scope') == science.WITHIN_MIGRATION:
        return routing.GEOMETRY_MANY if science.within_migration_eligible(meta) else None
    if ownership.enabled(manifest):
        return ownership.resource_policy(manifest, role, meta)
    artifact = manifest['artifacts'][role]
    family = next((f for f in artifact['families'] if affected(meta, [f])), None)
    if family is None:
        return None
    request = artifact['request']
    if role == 'candidate' and request == 'auto' and 'hidden' in manifest['artifacts']:
        hidden = manifest['artifacts']['hidden']['request']
        # The hidden arm binds the family it activates; a public candidate's
        # other eligible families keep their declared public policies.
        if routing.policy(hidden).get('family', family) == family:
            request = hidden
    if request == 'auto' and family == 'two':
        # An uninjected public invocation attests the public two-FE policy it
        # declares; configurations without the field keep gates v1.
        request = artifact.get('public_policy', routing.GATES)
    if family == 'two' and request in (*routing.REMAINING, *routing.GATE_RULES):
        return request
    if family == 'many' and request == routing.GEOMETRY_MANY:
        return request
    return 'incremental-' + family


def artifact_aliases(manifest):
    keys, aliases = {}, {}
    for role, artifact in manifest['artifacts'].items():
        key = (manifest['files'][artifact['path']], artifact['request'])
        aliases[role] = keys.setdefault(key, role)
    return aliases


def compare_artifacts(manifest, meta, snapshots):
    if ownership.enabled(manifest):
        return ownership.compare_artifacts(manifest, meta, snapshots)
    if diagnostic_mode(manifest):
        return diagnostic_tool(manifest).compare_science(manifest, meta, snapshots)
    candidate = snapshots['candidate']
    for role in ('original', 'incumbent', 'hidden'):
        if role not in snapshots:
            continue
        reference = snapshots[role]
        if manifest['scope'] == science.WITHIN_MIGRATION:
            science.within_migration_comparison(reference, candidate, meta)
            continue
        exact = (role == 'hidden' or manifest['scope'] == 'exact'
                 or role == 'incumbent' and not affected(meta, [manifest['scope']])
                 or not affected(meta, manifest['artifacts']['candidate']['families']))
        if exact:
            same = candidate[2]['fingerprint'] == reference[2]['fingerprint']
            if (not same and role != 'hidden' and manifest['scope'] != 'exact'
                    and science.derived_residual_only(reference[0], candidate[0], meta)):
                # SPEC §7: a derived residual row replaces a certified projection
                # without changing any published value.
                same = digest(candidate[1]) == digest(reference[1])
            require(same, role + ': unaffected science or selected-hidden equivalence changed')
        else:
            # The executed candidate policy owns its certificate contract.
            selected = (routing.GEOMETRY_MANY if resource_policy(manifest, 'candidate', meta) == routing.GEOMETRY_MANY
                        else routing.PORTFOLIO_UNSEEDED)
            science.compatibility(reference[0], candidate[0], reference[1], candidate[1], meta, 'auto',
                                  selected_policy=routing.policy(selected))


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
    numeric = ({meta['outputs'][name] for name in ('residual', 'xb', 'xbd', 'absorbed') if name in meta['outputs']}
               | set(meta.get('saved', [])))
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
    require(not diagnostic_mode(manifest) or not resume, 'diagnostic cannot resume or reset its budget')
    with Deadline(manifest['budget_s'], started) as deadline:
        return _run(manifest_path, output, manifest, deadline, shard, replica, resume, execute)


def _run(manifest_path, output, manifest, deadline, shard, replica, resume, execute):
    manifest_path, output = Path(manifest_path).resolve(), Path(output).resolve()
    require(manifest['schema'] == 1 and manifest['platform'] == platform.system(), 'wrong manifest/host platform')
    require(0 <= shard < manifest['shards'] and 0 <= replica < manifest['replicas'], 'invalid shard/replica')
    require('continuation' not in manifest or not resume and manifest['continuation']['hostname'] == socket.gethostname(),
            'continuation requires a fresh session on the original host')
    recovery = manifest.get('recovery')
    policy = cell_policy(manifest)
    validate_cell_policy(manifest)
    require(not policy or not resume, 'Mac cell policy requires a fresh session; budgets cannot be reset')
    require(not recovery or not resume and recovery['hostname'] == socket.gethostname(), 'recovery needs the original host and fresh session')
    require(output.is_relative_to((ROOT / 'target/tmp').resolve()), 'outputs belong under target/tmp/<task>')
    output.mkdir(parents=True, exist_ok=True)
    directory = output / f'run-{replica}-{shard}'
    path = directory / 'results.json'
    if ownership.enabled(manifest):
        ownership.validate(manifest)
        require(not resume, 'ownership qualification has no unbudgeted resume')
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
        minimum_hosts_per_cell(manifest)
        bindings = continuation_bindings(manifest, manifest_path)
        identities.update({p: stamp(p) for p in bindings.values()})
        validate_continuation(manifest, manifest_path)
        validate_recovery(manifest, manifest_path)
        bindings.update({name: resolve(manifest_path, name) for name in manifest['files']})
        files = {bindings[name]: sha for name, sha in manifest['files'].items()}
        identities.update(verify_sources(manifest['execution_sources']))
        result['execution_sources'] = manifest['execution_sources']
        for filename, expected in files.items():
            before = stamp(filename)
            require(digest(filename) == expected and stamp(filename) == before, 'input identity mismatch: ' + str(filename))
            identities[filename] = before
        if ownership.enabled(manifest):
            ownership.validate_bindings(manifest, manifest_path)
        result['verified_input_files'] = len(files)
        prior_cells, prior_root = {}, None
        if recovery:
            prior_cells, prior_root = recovery_prior(manifest, manifest_path)
            result['cases'] = [{**copy.deepcopy(prior_cells[cid]), 'reused': True} for cid in recovery['reused_cells']]
        if policy:
            result.update(discarded_attempts=[], retry_cells=[], native_calls_started=0)
        from qualify_scc import HostMonitor, MacMonitor
        monitor = HostMonitor if manifest['platform'] == 'Linux' else MacMonitor
        host = monitor(directory / f"host-{len(result['failures'])}", max(manifest['threads']), **({'recovery': True} if policy else {}))
        require(not policy or host.process_policy == policy['process_policy'], 'recovery observer policy changed')
        with deadline.observing(host):
            for index, item in enumerate(manifest['cases']):
                if index % manifest['shards'] != shard:
                    continue
                meta = metadata(manifest, item, manifest_path)
                signatures, first_worker, output_files = {}, {}, []
                pending = [(workers, 0) for workers in manifest['threads']]
                while pending:
                    workers, attempt = pending.pop(0)
                    cell_id = f"{item['id']}-t{workers}"
                    completed = next((c for c in result['cases'] if c['id'] == cell_id), None)
                    if completed is not None:
                        signatures[workers] = completed['worker_signatures']
                        continue
                    phase = 'setup'
                    cell = {'id': cell_id, 'family': item['family'], 'status': 'running', 'blocks': [], 'warmups': [],
                            'host_attempt': len(result['failures'])}
                    result['cases'].append(cell)
                    if policy:
                        cell['attempt'] = attempt
                        if attempt:
                            cell['quiet_wait'] = quiet_wait
                    work = directory / (f"{cell_id}-attempt-{len(result['failures'])}" + (f'-{attempt}' if policy else ''))
                    work.mkdir(exist_ok=False)
                    aliases, prepared, snapshots, requests, request_arguments = {}, {}, {}, {}, {}
                    aliases = artifact_aliases(manifest)
                    for role, artifact in manifest['artifacts'].items():
                        if aliases[role] != role:
                            continue
                        request = work / (role + '.case')
                        source = resolve(manifest_path, item['input'])
                        if ownership.enabled(manifest):
                            ownership.prepare(source, request, artifact['request'], workers)
                        elif artifact['request'] == 'preserve':
                            prepare_case(source, request, workers)
                        else:
                            routing.prepare(source, request, 'baseline' if artifact['request'] == 'auto' else artifact['request'], workers)
                        prepared[role] = request
                        with request.open('rb') as stream:
                            _, arguments = case_header(stream)
                        requests[role] = dict(a.decode().split('=', 1) for a in arguments[1:])
                        request_arguments[role] = arguments
                        identities[request] = stamp(request)
                    cell['aliases'] = aliases
                    persist()

                    def call(role, name, traced):
                        nonlocal phase
                        calibration = manifest.get('diagnostic') == 'two-fe-remaining-calibration-v1'
                        traced = traced or calibration
                        check_files()
                        if host is not None:
                            host.check()
                        artifact = manifest['artifacts'][role]
                        target, log = work / (name + '.json'), work / (name + '.log')
                        env = ['env', *[v for key in ENVIRONMENT for v in ('-u', key)],
                               *[key + '=1' for key in THREAD_ENV]]
                        if traced:
                            env += ['FEREG_SOLVER_TRACE=1']
                        if calibration:
                            env += ['FEREG_PHASE_TIMING=1']
                        if item['memory_limit_bytes'] is not None:
                            env += ['FEREG_MEMORY_LIMIT_BYTES=' + str(item['memory_limit_bytes'])]
                        command = [*env, str(resolve(manifest_path, manifest['harness'])),
                                   str(resolve(manifest_path, artifact['path'])), '--replay', str(prepared[role]), str(target)]
                        timeout = remaining()
                        phase = 'native'
                        if policy:
                            require(result['native_calls_started'] < policy['new_call_cap'], 'recovery native-call cap exhausted')
                            result['native_calls_started'] += 1
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
                        if traced and manifest['scope'] == science.WITHIN_MIGRATION:
                            detail.update(science.within_migration_trace(log.read_text(), meta),
                                          trace_log=str(log.relative_to(directory)))
                        elif traced and ownership.enabled(manifest):
                            with source.open('rb') as stream:
                                _, source_args = case_header(stream)
                            detail.update(ownership.resource_detail(manifest, role, meta, log.read_text(),
                                          request_arguments[role], source_args, workers))
                        elif traced and artifact['families']:
                            eligible = affected(meta, artifact['families'])
                            if eligible:
                                family = next(f for f in artifact['families'] if affected(meta, [f]))
                                if diagnostic_mode(manifest):
                                    resources_detail = diagnostic_tool(manifest).resource_diagnostics(
                                        log.read_text(), role, artifact['revision'], family=family)
                                else:
                                    selected = resource_policy(manifest, role, meta)
                                    resources_detail = science.resource_diagnostics(log.read_text(), selected,
                                        False, {}, expected_preset=routing.policy(selected)['injection'])
                                    if selected in routing.GATE_RULES:
                                        import two_fe_policy_development as development
                                        resources_detail['gate_events'] = development.gate_events(
                                            log.read_text(), routing.GATE_RULES[selected])
                                detail.update(resources_detail)
                        if ownership.enabled(manifest):
                            detail.update(raw_result=raw, result_sha256=digest(target), log_sha256=digest(log),
                                          columns_sha256=digest(columns), target=str(target.relative_to(directory)),
                                          log=str(log.relative_to(directory)))
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
                    current_signatures = {role: worker_signature(s[0]) for role, s in conceptual.items()}
                    require(workers not in signatures or signatures[workers] == current_signatures,
                            'same-worker retry discrete decisions changed')
                    if policy:
                        for previous_attempt in result['discarded_attempts']:
                            if previous_attempt['id'] == cell_id:
                                require({c['execution']: c['fingerprint'] for c in previous_attempt['warmups']}
                                        == {c['execution']: c['fingerprint'] for c in cell['warmups']}, 'same-worker retry science changed')
                    signatures[workers] = current_signatures
                    require(all(v == signatures[workers] for v in signatures.values()), 'cross-worker discrete decisions changed')
                    if first_worker:
                        for role, current in conceptual.items():
                            prior = first_worker[role]
                            cmgschur.scientific_comparison(prior[0], current[0], prior[1], current[1], meta)
                            if resource_policy(manifest, role, meta) in routing.GATE_RULES:
                                import two_fe_policy_development as development
                                require(development.semantic_signature(current[2]) == development.semantic_signature(prior[2]),
                                        'cross-worker gate decisions changed')
                    first_worker = first_worker or conceptual
                    cell['worker_signatures'] = signatures[workers]
                    saved = {role: retained_snapshot(work / ('reference-' + role), s[0], s[1], meta)
                             for role, s in snapshots.items()}
                    cell['references'] = {role: str((work / ('reference-' + alias)).relative_to(directory))
                                          for role, alias in aliases.items()}
                    cell['reference_files'] = {role: saved[alias] for role, alias in aliases.items()}
                    if policy:
                        for prior_cell in result['cases'] + result['discarded_attempts']:
                            if prior_cell is cell or not same_source_case(prior_cell, cell):
                                continue
                            require(prior_cell['worker_signatures'] == cell['worker_signatures'], 'retained cross-worker decisions changed')
                            root = prior_root if prior_cell.get('reused') else directory
                            for role in aliases:
                                compare_saved(root / prior_cell['references'][role], directory / cell['references'][role], meta)
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
                            if not ownership.enabled(manifest):
                                columns.with_suffix('').unlink()
                                (work / f'block-{block}-{number}.log').unlink()
                        row['comparisons'] = {comparison_key(manifest, role, target): score(row['calls'], aliases[role], aliases[target])
                                              for role, target in comparison_pairs(manifest)}
                        row['status'] = 'complete'
                        persist()
                    cell['status'] = 'complete'
                    hits = []
                    if policy:
                        start, end = cell_records(cell, manifest, index, block_indices(manifest, cell_id))
                        hits = host.interference_between(start, end)
                        cell['eligibility'] = {'start_monotonic_ns': start, 'end_monotonic_ns': end, 'interference': hits}
                        if hits:
                            cell['status'] = 'interfered'
                    for request in prepared.values():
                        identities.pop(request)
                        request.unlink()
                    output_files.extend(snapshot[1] for snapshot in snapshots.values())
                    persist()
                    if hits:
                        result['cases'].remove(cell)
                        result['discarded_attempts'].append(cell)
                        persist()
                        require(attempt < policy['max_cell_retries'] and len(result['retry_cells']) < policy['max_retry_cells'],
                                'interference retry limit exhausted; retained completed cells')
                        quiet_wait = host.await_quiet(deadline_seconds=min(policy['quiet_timeout_s'], deadline.check()))
                        result['retry_cells'].append(cell_id)
                        pending.insert(0, (workers, attempt+1))
                for columns in output_files:
                    columns.unlink()
            phase = 'setup'
            check_files()
        if host is not None:
            result['host'] = host.report()
            require(result['host']['isolation_ok'], 'host observation did not complete')
            if policy or manifest['platform'] == 'Linux' and 'observations' in result['host']:
                result['host_observations_sha256'] = digest(directory / 'host-0' / result['host']['observations'])
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


def same_source_case(left, right):
    """Worker agreement belongs to one actual input, not its generator family."""
    return left['id'].rsplit('-t', 1)[0] == right['id'].rsplit('-t', 1)[0]


def accounting(text, host, *, normalize=False):
    values = {}
    for line in text.splitlines():
        pair = line.split(None, 1)
        if len(pair) == 2:
            require(pair[0] not in values, 'accounting must contain one task')
            values[pair[0]] = pair[1].strip()
    def task_id(value):
        require(isinstance(value, str) and (value in ('0', 'undefined') or
                re.fullmatch(r'[1-9][0-9]*', value) is not None), 'invalid SGE task identity')
        return '0' if value == 'undefined' else value
    require(values.get('jobnumber') == str(host['job_id']) and task_id(values.get('taskid')) == task_id(str(host['task_id']))
            and physical_host(values.get('hostname', ''), normalize) == physical_host(host['hostname'], normalize)
            and values.get('project') == 'welfgr' and values.get('failed', '').split()[0:1] == ['0']
            and values.get('exit_status') == '0' and values.get('end_time'), 'missing/failed SGE accounting')


def validate_allocation_amendment(path, manifest_path, output, verify_file):
    """Bind two source cohorts without changing any completed observation."""
    path = Path(path).resolve(strict=True)
    before = stamp(path)
    descriptor = read(path)
    descriptor_sha = digest(path)
    require(stamp(path) == before, 'allocation amendment changed while reading')
    verify_file(path, descriptor_sha)
    require(set(descriptor) == {'schema', 'kind', 'original_plan', 'amended_plan', 'source_roots',
            'completed_replicas', 'amended_replicas', 'completed_results', 'completed_observations', 'source_changes'}
            and descriptor['schema'] == 1 and descriptor['kind'] == 'linux-allocation-amendment-v1',
            'invalid allocation amendment schema')
    plans, paths = {}, {}
    for name in ('original', 'amended'):
        binding = descriptor[name + '_plan']
        require(set(binding) == {'path', 'sha256'} and routing.sha(binding['sha256']), 'invalid allocation plan binding')
        paths[name] = resolve(path, binding['path'])
        verify_file(paths[name], binding['sha256'])
        plans[name] = read(paths[name])
    old, new = plans['original'], plans['amended']
    require(paths['amended'] == Path(manifest_path).resolve(strict=True), 'report must use the bound amended plan')
    require(old['platform'] == 'Linux' and old['schema'] == 1 and minimum_hosts_per_cell(old) is not None
            and not any(k in old for k in ('diagnostic', 'continuation', 'recovery', 'mac_cell_policy')),
            'allocation amendment requires an original Linux qualification')
    require({**old, 'execution_sources': new['execution_sources']} == new,
            'allocation amendment changed inputs, artifacts, science, schedule or budgets')
    sources = execution_sources()
    require(set(old['execution_sources']) == set(new['execution_sources']) == set(sources),
            'allocation amendment source coverage changed')
    changes = {name: {'before': old['execution_sources'][name], 'after': new['execution_sources'][name]}
               for name in sources if old['execution_sources'][name] != new['execution_sources'][name]}
    require(changes == descriptor['source_changes'] and 'qualify_scc' in changes
            and set(changes) <= {'qualify', 'qualify_scc'}, 'unapproved allocation source delta')
    require(set(descriptor['source_roots']) == {'original', 'amended'}, 'invalid allocation source roots')
    for name, plan in plans.items():
        root = resolve(path, descriptor['source_roots'][name])
        for module, source in sources.items():
            expected = plan['execution_sources'][module]
            require(routing.sha(expected), 'invalid archived execution hash')
            verify_file(root / source.relative_to(ROOT), expected)
        for relative, expected in plan['files'].items():
            file = resolve(paths[name], relative)
            require(file == resolve(paths['amended'], relative), 'allocation input alias changed')
            verify_file(file, expected)
    completed, amended = descriptor['completed_replicas'], descriptor['amended_replicas']
    require(all(isinstance(ids, list) and ids and all(type(i) is int for i in ids)
                and ids == sorted(set(ids)) for ids in (completed, amended))
            and not set(completed) & set(amended)
            and sorted(completed + amended) == list(range(old['replicas'])),
            'allocation replica partition is incomplete or overlapping')
    expected_results = {f'run-{replica}-{shard}/results.json'
                        for replica in completed for shard in range(old['shards'])}
    require(set(descriptor['completed_results']) == expected_results, 'completed allocation result coverage changed')
    for relative, expected in descriptor['completed_results'].items():
        require(routing.sha(expected), 'invalid completed allocation result hash')
        verify_file(Path(output) / relative, expected)
    expected_observations = {name.replace('results.json', 'host-0/host-observations.jsonl') for name in expected_results}
    require(set(descriptor['completed_observations']) == expected_observations, 'completed allocation observer coverage changed')
    for relative, expected in descriptor['completed_observations'].items():
        require(routing.sha(expected), 'invalid completed observer hash')
        verify_file(Path(output) / relative, expected)
    return {'plans': plans, 'completed': set(completed),
            'record': {'descriptor_sha256': descriptor_sha, **descriptor}}


def validate_allocation_host(result, directory, accounting_path, expected_hash, amended, threads, verify_file):
    """Recheck both allocation contracts from sealed raw observer evidence."""
    import qualify_scc as scc
    report, host = result['host'], result['host']['host']
    expected_pe = 'mpi_28_tasks_per_node_u' if amended else 'omp28'
    require(host.get('pe', 'omp28') == expected_pe and host['slots'] == 28
            and report['errors'] == [] and report['observations'] == 'host-observations.jsonl',
            'allocation cohort PE or observer mismatch')
    observer = directory / 'host-0' / report['observations']
    require(routing.sha(expected_hash), 'missing allocation observer binding')
    verify_file(observer, expected_hash)
    events = [json.loads(line) for line in observer.read_text().splitlines()]
    require(all(e['kind'] in ('reservation', 'census', 'process') and not e.get('competing_compute') for e in events)
            and all(sum(e['kind'] == kind for e in events) >= 2 for kind in ('reservation', 'census', 'process')),
            'allocation observer incomplete or reports interference')
    try:
        for event in events:
            if event['kind'] == 'reservation':
                require(scc.validate_reservation(event['raw'], host, max(threads)) == host['queue'], 'allocation queue changed')
            elif event['kind'] == 'census':
                scc.validate_census(event['raw']['job'], event['raw']['queues'], host)
        accounting_text = accounting_path.read_text()
        values = scc.fields(accounting_text)
    except scc.OperationalError as error:
        raise ValueError('allocation evidence: ' + str(error)) from error
    require(values.get('granted_pe') == expected_pe and values.get('slots') == '28', 'accounted allocation PE/slots mismatch')
    verify_file(accounting_path, digest(accounting_path))
    calls = [call for cell in result['cases'] for call in cell_calls(cell)]
    span = span_evidence(events, min(c['started_monotonic_ns'] for c in calls),
                         max(c['finished_monotonic_ns'] for c in calls))
    require(not span['interference'], 'allocation process observation does not cover calls')


def report(manifest_path, output, accounting_dir=None, *, allocation_amendment=None):
    manifest, output = read(manifest_path), Path(output)
    diagnostic = diagnostic_mode(manifest)
    recovery = manifest.get('recovery')
    policy = cell_policy(manifest)
    old_cells, old_root, old_events = {}, None, []
    results, errors, comparisons, workers, snapshots, metadata_cache, gate_signatures = [], [], {}, {}, {}, {}, {}
    source_stamps = {}
    minimum_hosts = None
    prior_bindings = {}
    verified_files = {}
    calibration_result_sha256 = None
    allocation = None

    def verify_file(path, expected):
        path = Path(path).absolute()
        resolved = path.resolve(strict=True)
        prior_bindings[str(path)] = resolved
        if resolved not in verified_files:
            before = stamp(resolved)
            verified_files[resolved] = digest(resolved)
            require(stamp(resolved) == before, 'report file changed while hashing')
            source_stamps[resolved] = before
        require(verified_files[resolved] == expected and stamp(resolved) == source_stamps[resolved], 'report file identity mismatch: '+str(path))
    try:
        minimum_hosts = minimum_hosts_per_cell(manifest)
        validate_cell_policy(manifest)
        prior_bindings = continuation_bindings(manifest, manifest_path)
        if recovery:
            prior_bindings.update({recovery[k]['path']: resolve(manifest_path, recovery[k]['path'])
                                   for k in ('prior_plan', 'prior_result', 'prior_host', 'prior_review')})
        source_stamps.update({p: stamp(p) for p in prior_bindings.values()})
        validate_continuation(manifest, manifest_path)
        validate_recovery(manifest, manifest_path)
        if recovery:
            old_cells, old_root = recovery_prior(manifest, manifest_path)
            old_events = [json.loads(line) for line in resolve(manifest_path, recovery['prior_host']['path']).read_text().splitlines()]
        source_stamps.update(verify_sources(manifest['execution_sources']))
        if allocation_amendment is not None:
            allocation = validate_allocation_amendment(allocation_amendment, manifest_path, output, verify_file)
        if policy or ownership.enabled(manifest):
            for name, expected_hash in manifest['files'].items():
                verify_file(Path(manifest_path).parent / name, expected_hash)
        if ownership.enabled(manifest):
            ownership.validate_bindings(manifest, manifest_path)
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append('execution sources: ' + str(error))
    aliases = artifact_aliases(manifest)
    cases = {item['id']: (index, item) for index, item in enumerate(manifest['cases'])}
    for replica in range(manifest['replicas']):
        for shard in range(manifest['shards']):
            path = output / f'run-{replica}-{shard}/results.json'
            try:
                before = stamp(path)
                result = read(path)
                expected_manifest = (allocation['plans']['original' if replica in allocation['completed'] else 'amended']
                                     if allocation else manifest)
                if allocation:
                    require(stamp(path) == before, 'allocation result changed while reading')
                    verify_file(path, digest(path))
                    require(result['failures'] == [] and not result.get('previous_hosts'),
                            'allocation amendment cannot hide failed or reused attempts')
                if manifest.get('diagnostic') in ('two-fe-remaining-calibration-v1', 'two-fe-gates-development-v1'):
                    calibration_result_sha256 = digest(path)
                    require(stamp(path) == before, 'calibration result changed while hashing')
                    source_stamps[path] = before
                require(result['manifest_sha256'] == routing.identity(expected_manifest)
                        and (result['replica'], result['shard']) == (replica, shard)
                        and result['status'] == 'complete', 'missing, changed or failed shard')
                require(result.get('execution_sources') == expected_manifest['execution_sources'], 'unverified execution sources')
                require('continuation' not in manifest or result['hostname'] == manifest['continuation']['hostname'],
                        'continuation host changed')
                require(result.get('budget_compliant') is True
                        and 0 <= result['elapsed_s'] < manifest['budget_s'], 'unverified run budget')
                require(result['host']['isolation_ok'], 'missing host observation')
                current_events = []
                if policy:
                    require(not result['failures'] and result['hostname'] == result['host']['host']['hostname']
                            and (not recovery or result['hostname'] == recovery['hostname']) and result['host']['cell_eligibility_required'] is True
                            and not result['host']['errors'] and result['host']['process_policy'] == policy['process_policy'],
                            'invalid recovery host/failure state')
                    raw_host = path.parent / 'host-0' / result['host']['observations']
                    verify_file(raw_host, result['host_observations_sha256'])
                    current_events = [json.loads(line) for line in raw_host.read_text().splitlines()]
                    require(not any(r['kind'] == 'error' for r in current_events), 'recovery contains an observer fault')
                    rejected = result['discarded_attempts']
                    require(result['retry_cells'] == [c['id'] for c in rejected]
                            and len(set(result['retry_cells'])) == len(rejected) <= policy['max_retry_cells'], 'invalid recovery retries')
                    for cell in rejected:
                        require(cell['status'] == 'interfered' and cell['attempt'] == 0
                                and cell['id'] in policy['fresh_cells']
                                and set(cell['references']) == set(cell['reference_files']) == set(aliases), 'invalid rejected attempt')
                        index = cases[cell['id'].rsplit('-t', 1)[0]][0]
                        start, end = cell_records({**cell, 'status': 'complete'}, manifest, index, block_indices(manifest, cell['id']))
                        evidence = span_evidence(current_events, start, end)
                        require(evidence['interference'] and all(cell['eligibility'][k] == evidence[k]
                                for k in ('start_monotonic_ns', 'end_monotonic_ns', 'interference')), 'unjustified cell retry')
                        accepted = next(c for c in result['cases'] if c['id'] == cell['id'])
                        require(cell['worker_signatures'] == accepted['worker_signatures']
                                and {c['execution']: c['fingerprint'] for c in cell['warmups']}
                                == {c['execution']: c['fingerprint'] for c in accepted['warmups']}, 'retry science changed')
                        wait = accepted['quiet_wait']
                        require(end <= wait['started_monotonic_ns'] < wait['finished_monotonic_ns']
                                <= accepted['warmups'][0]['started_monotonic_ns']
                                and wait['finished_monotonic_ns']-wait['started_monotonic_ns'] <= policy['quiet_timeout_s']*1e9
                                and len(wait['clean_intervals']) == 2, 'invalid retry quiet-wait chronology')
                        boundary = wait['started_monotonic_ns']
                        for interval_index, quiet_interval in enumerate(wait['clean_intervals']):
                            a, b = quiet_interval['start_monotonic_ns'], quiet_interval['end_monotonic_ns']
                            require(boundary <= a < b <= wait['finished_monotonic_ns']
                                    and (interval_index == 0 or a == boundary)
                                    and any(r['kind'] == 'process' and r['monotonic_ns'] == b for r in current_events)
                                    and not span_evidence(current_events, a, b)['interference'], 'retry lacks two clean intervals')
                            boundary = b
                        item = cases[cell['id'].rsplit('-t', 1)[0]][1]
                        meta = routing_cases.inspect_case(resolve(manifest_path, item['input']), item['family'])
                        for role, relative in cell['references'].items():
                            base = path.parent / relative
                            require(base.resolve().is_relative_to(path.parent.resolve()), 'rejected reference outside job')
                            for suffix, name in (('.json', 'head'), ('.npz', 'outputs')):
                                verify_file(base.with_suffix(suffix), cell['reference_files'][role][name])
                            raw = read(base.with_suffix('.json'))
                            validate_result(raw)
                            require(worker_signature(raw) == cell['worker_signatures'][role],
                                    'rejected attempt discrete science changed')
                            compare_saved(base, path.parent / accepted['references'][role], meta)
                    calls = sum(len(cell_calls(c)) for c in [*result['cases'], *rejected] if not c.get('reused'))
                    require(calls == result['native_calls_started'] <= policy['new_call_cap'], 'recovery call budget changed')
                if manifest['platform'] == 'Linux':
                    host = result['host']['host']
                    require(physical_host(result['hostname'], minimum_hosts is not None)
                            == physical_host(host['hostname'], minimum_hosts is not None),
                            'recorded host differs from accounted host')
                    require(accounting_dir is not None, 'SGE accounting is required')
                    accounting_path = Path(accounting_dir) / f"{host['job_id']}.{host['task_id']}.txt"
                    accounting(accounting_path.read_text(), host,
                               normalize=minimum_hosts is not None)
                    if allocation:
                        completed = replica in allocation['completed']
                        observer_key = f'run-{replica}-{shard}/host-0/host-observations.jsonl'
                        observer_hash = (allocation['record']['completed_observations'][observer_key] if completed
                                         else result.get('host_observations_sha256'))
                        validate_allocation_host(result, path.parent, accounting_path, observer_hash,
                                                 not completed, manifest['threads'], verify_file)
                expected = {f"{c['id']}-t{t}" for i, c in enumerate(manifest['cases'])
                            if i % manifest['shards'] == shard for t in manifest['threads']}
                require(len(result['cases']) == len(expected) and {c['id'] for c in result['cases']} == expected,
                        'incomplete or duplicate workload coverage')
                last_finished = 0
                for cell in result['cases']:
                    indices = list(block_indices(manifest, cell['id']))
                    require(cell['status'] == 'complete' and [r['index'] for r in cell['blocks']] == indices, 'incomplete cell')
                    hosts = [*result.get('previous_hosts', []), result['host']]
                    reference_root = path.parent
                    if policy:
                        index = cases[cell['id'].rsplit('-t', 1)[0]][0]
                        start, end = cell_records(cell, manifest, index, indices)
                        if cell.get('reused'):
                            require(recovery and cell['id'] in recovery['reused_cells'] and {k: v for k, v in cell.items() if k != 'reused'}
                                    == old_cells[cell['id']], 'retained cell changed')
                            evidence = span_evidence(old_events, start, end)
                            require(evidence == recovery['retained_spans'][cell['id']], 'retained span changed')
                            reference_root = old_root
                        else:
                            require(cell['id'] in policy['fresh_cells'] and cell['attempt'] == int(cell['id'] in result['retry_cells']),
                                    'fresh recovery cell attempt changed')
                            evidence = span_evidence(current_events, start, end)
                            require(all(cell['eligibility'][k] == evidence[k] for k in
                                        ('start_monotonic_ns', 'end_monotonic_ns', 'interference')), 'cell eligibility changed')
                        require(not evidence['interference'], 'interfered cell cannot enter comparison')
                    else:
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
                    if key not in metadata_cache:
                        metadata_cache[key] = routing_cases.inspect_case(resolve(manifest_path, item['input']), item['family'])
                    policy_meta = metadata_cache[key]
                    if ownership.enabled(manifest):
                        ownership.verify_report_cell(manifest, item, manifest_path, reference_root, cell, verify_file)
                    for call in cell['warmups']:
                        if manifest['scope'] == science.WITHIN_MIGRATION:
                            trace = (reference_root / call['trace_log']).resolve(strict=True)
                            require(trace.is_relative_to(reference_root.resolve()), 'migration trace outside job')
                            expected_trace = science.within_migration_trace(trace.read_text(), policy_meta)
                            require(all(call.get(k) == v for k, v in expected_trace.items()),
                                    'retained within migration path evidence changed')
                        if resource_policy(manifest, call['execution'], policy_meta) in routing.GATE_RULES:
                            import two_fe_policy_development as development
                            signature = development.semantic_signature(call)
                            policy_key = (key, call['execution'])
                            require(policy_key not in gate_signatures or gate_signatures[policy_key] == signature,
                                    'retained cross-host/worker gate decisions changed')
                            gate_signatures[policy_key] = signature
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
                        current = reference_root / relative
                        require(current.resolve().is_relative_to(reference_root.resolve()), 'reference outside job directory')
                        for suffix, name in (('.json', 'head'), ('.npz', 'outputs')):
                            if policy:
                                verify_file(current.with_suffix(suffix), cell['reference_files'][role][name])
                            else:
                                require(digest(current.with_suffix(suffix)) == cell['reference_files'][role][name],
                                        'retained scientific output changed')
                        raw = read(current.with_suffix('.json'))
                        validate_result(raw)
                        if policy:
                            require(worker_signature(raw) == cell['worker_signatures'][role], 'retained discrete science changed')
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
                        for role, target in comparison_pairs(manifest):
                            value = score(row['calls'], cell['aliases'][role], cell['aliases'][target])
                            comparisons.setdefault((cell['id'], comparison_key(manifest, role, target)), []).append(
                                (physical_host(result['hostname'], minimum_hosts is not None), value))
                results.append(result)
            except (OSError, ValueError, KeyError, TypeError) as error:
                errors.append(f'replica {replica}, shard {shard}: {error}')
    if minimum_hosts is not None:
        for item in manifest['cases']:
            for worker in manifest['threads']:
                cell = f"{item['id']}-t{worker}"
                for role, _ in comparison_pairs(manifest):
                    count = len({host for host, _ in comparisons.get((cell, role), [])})
                    if count < minimum_hosts:
                        errors.append(f'{cell}, {role}: {count} distinct physical hosts; requires {minimum_hosts}')
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
        row = {'case': cell, 'reference': role, 'status': state, 'excess_seconds': bound,
               'median_ratio': statistics.median(v['ratio'] for _, v in values)}
        if minimum_hosts is not None:
            row.update(distinct_hosts=hosts, distinct_host_count=len(hosts))
            if len(hosts) < minimum_hosts:
                row['status'] = 'insufficient_hosts'
        rows.append(row)
    try:
        require(all(stamp(path) == saved for path, saved in source_stamps.items()),
                'execution sources changed during report')
        require(all(resolve(manifest_path, name) == p for name, p in prior_bindings.items()), 'prior evidence alias changed')
    except (OSError, ValueError) as error:
        errors.append(str(error))
    if diagnostic and manifest['platform'] == 'Linux':
        required_hosts = 1 if manifest['diagnostic'] in (
            'two-fe-remaining-calibration-v1', 'two-fe-gates-development-v1') else 6
        if len({r['hostname'].split('.')[0] for r in results}) != required_hosts:
            errors.append(f'diagnostic requires {required_hosts} distinct accounted physical hosts')
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
    if ownership.enabled(manifest):
        summary['qualification'] = ownership.MODE
        summary['ownership_binding'] = manifest['ownership_binding']
        summary['activation_qualified'] = False
        summary['interpretation'] = 'Performance qualification only; selected hidden/public equality and native feature/transaction gates remain separate.'
        if manifest['platform'] == 'Darwin':
            summary['recovery_limit'] = '8526 total calls funds the fixed 58-cell schedule only; external interference is retained, with no additional native-call allowance.'
    if diagnostic:
        summary.update(status='diagnostic_complete' if complete else 'needs_discussion',
                       diagnostic=manifest['diagnostic'], activation_qualified=False,
                       target=comparison_pairs(manifest)[0][1],
                       next_action='Review this bounded diagnostic; no activation qualification or automatic sampling.')
        if manifest['diagnostic'] == 'two-fe-remaining-calibration-v1':
            summary.pop('target')
            summary['targets'] = [target for _, target in comparison_pairs(manifest)]
            summary['calibration_result_sha256'] = calibration_result_sha256
        elif manifest['diagnostic'] == 'two-fe-gates-development-v1':
            summary['development_result_sha256'] = calibration_result_sha256
            summary['policy_binding'] = manifest['policy_binding']
    if 'min_distinct_hosts_per_cell' in manifest:
        summary['min_distinct_hosts_per_cell'] = manifest['min_distinct_hosts_per_cell']
    if allocation:
        summary['allocation_amendment'] = allocation['record']
    if 'continuation' in manifest:
        state = manifest['continuation']
        summary['continuation'] = {k: state[k] for k in ('prior_plan', 'prior_result', 'prior_calls', 'prior_elapsed_s',
                                  'prior_complete_blocks', 'prior_partial_blocks', 'warmup_calls', 'timed_calls', 'original_call_cap')}
        summary['continuation'].update(prior_status='blocked', prior_observations_used=False,
            planned_new_calls=state['warmup_calls']+state['timed_calls'],
            planned_total_calls=state['prior_calls']+state['warmup_calls']+state['timed_calls'],
            cumulative_elapsed_s=state['prior_elapsed_s']+results[0]['elapsed_s'] if len(results) == 1 else None)
    if recovery:
        summary['recovery'] = {k: recovery[k] for k in ('prior_plan', 'prior_result', 'prior_host', 'prior_review', 'scientific_review_limits', 'reused_cells', 'fresh_cells',
                              'prior_calls', 'prior_elapsed_s', 'baseline_calls', 'new_call_cap', 'max_cell_retries', 'max_retry_cells')}
        summary['recovery'].update(selection='complete cells with sealed external evidence; no timing-based selection',
            new_calls=results[0]['native_calls_started'] if results else None,
            cumulative_calls=recovery['prior_calls']+results[0]['native_calls_started'] if results else None,
            cumulative_elapsed_s=recovery['prior_elapsed_s']+results[0]['elapsed_s'] if results else None,
            discarded_attempts=results[0]['discarded_attempts'] if results else [])
    if 'mac_cell_policy' in manifest:
        summary['mac_cell_policy'] = {**policy,
            'selection': 'complete cells with sealed external evidence; no timing-based selection',
            'new_calls': results[0]['native_calls_started'] if results else None,
            'discarded_attempts': results[0]['discarded_attempts'] if results else []}
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
    recovery = commands.add_parser('recovery-plan')
    recovery.add_argument('manifest', type=Path)
    recovery.add_argument('results', type=Path)
    recovery.add_argument('--host', type=Path, required=True)
    recovery.add_argument('--review', type=Path, required=True)
    recovery.add_argument('--output', type=Path, required=True)
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
            command.add_argument('--allocation-amendment', type=Path)
    args = parser.parse_args()
    if args.action == 'recovery-plan':
        recovery_plan(args.manifest, args.results, args.host, args.review, args.output)
        print('Planned clean-cell reuse and bounded Mac cell recovery; no native calls.')
        return 0
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
    result = report(args.manifest, args.output, args.accounting, allocation_amendment=args.allocation_amendment)
    print(result['status'])
    return 0 if result['status'] in {'passed', 'smoke_complete'} else 1


if __name__ == '__main__':
    raise SystemExit(main())
