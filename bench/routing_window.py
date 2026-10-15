#!/usr/bin/env python3
"""Retrospective isolation certificate for the first two sequential SCC rounds.

This is a distinct, narrower certificate, never a replacement full-collection
receipt. Eligibility uses source order, monotonic bounds and external observations;
all three rounds must still pass the original scientific validators. No score or
ranking is calculated. Pure replay runs in a fresh interpreter with the original
frozen receipt generator's sibling modules, not the current host implementation.
"""
from __future__ import annotations

import argparse
import importlib
import hashlib
import json
import math
from pathlib import Path
import re
import shlex
import subprocess
import sys

KIND = 'routing_scc_window_certificate'
REPLAY_TIMEOUT_S = 300
PINNED_SOURCES = {
    'routing_discovery.py': '232a46e6bef4a69a12a9a9eef1783c364a4df809ff28ce25b493f91631d06a59',
    'routing_run.py': 'e720375050281f609f02eef2e894fbb2fd8300a4b39a0bfbfb8de986b9ac7e15',
    'scc_routing_host.py': '8ea1e9b3361a3c161afb409e5b879c9354da819bf26773b6b9031188bf90e087',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def checked(ref):
    require(isinstance(ref, dict) and set(ref) == {'path', 'sha256'}, 'invalid original file binding')
    path = Path(ref['path']).resolve(strict=True)
    require(hashlib.sha256(path.read_bytes()).hexdigest() == ref['sha256'], 'original file changed')
    return path


def source_semantics(plan, old, host):
    """The clock proof is specific to the reviewed, immutable old implementation.

    Its collect() synchronously completes range(3) invokes in order; run() starts
    its monotonic duration inside invoke and records elapsed_s in its finally,
    before returning. Collection completion is timestamped after all invokes.
    Source pins establish those semantics; every other frozen file is checked by
    the original validate_plan(). These are source identities, never data fences.
    """
    host.check_sources(plan)
    sources = {}
    for module in (old, old.routing_run, host):
        ref = old.reference(module.__file__)
        require(ref['sha256'] == PINNED_SOURCES.get(Path(ref['path']).name)
                and plan['frozen_files'].get(ref['path']) == ref['sha256'],
                'window clock proof requires the reviewed original source')
        sources[ref['path']] = ref['sha256']
    return sources


def elapsed_bound(collection, third):
    duration = third.get('elapsed_s')
    require(type(duration) in (int, float) and math.isfinite(duration) and duration >= 2,
            'missing finite round-three elapsed duration')
    # Discard a full second plus the fractional second; float monotonic seconds
    # and integer monotonic nanoseconds then give a conservative upper bound.
    lower_ns = (math.floor(duration) - 1) * 1_000_000_000
    end = collection['completed_monotonic_ns'] - lower_ns
    require(collection['started_monotonic_ns'] < end < collection['completed_monotonic_ns'],
            'invalid round-three elapsed clock bound')
    return {'completed_monotonic_ns': end, 'round': collection['rounds'][2],
            'elapsed_s': duration, 'conservative_duration_ns': lower_ns}


def process_bound(plan, collection, path, host):
    """An actual direct-child round-three native call attests prior completion."""
    directory = Path(collection['rounds'][2]['path']).parent
    owner_start, found = None, None
    for line, row in enumerate(host.rows(path), 1):
        if row.get('kind') != 'snapshot':
            continue
        current = host.parsed_processes(row)
        owners = [p for p in current['processes'] if p['pid'] == collection['collection_pid']]
        if owner_start is None and owners:
            owner_start = owners[0]['start']
        if not owners or owners[0]['start'] != owner_start:
            continue
        for process in current['processes']:
            if process['ppid'] != collection['collection_pid'] or not process.get('command'):
                continue
            args = shlex.split(process['command'])
            if (len(args) == 5 and args[0] == plan['harness'] and args[1] in plan['plugins'].values()
                    and args[2] == '--replay' and Path(args[3]).parent == directory
                    and re.fullmatch(r'input-\d+-\d+\.case', Path(args[3]).name)
                    and Path(args[4]) == directory / 'current.json'):
                finish = current['finished_monotonic_ns']
                require(collection['started_monotonic_ns'] <= current['monotonic_ns'] <= finish
                        <= collection['completed_monotonic_ns'], 'child attestation outside collection')
                if found is None:
                    found = {'completed_monotonic_ns': finish, 'line': line,
                             'pid': process['pid'], 'ppid': process['ppid'], 'start': process['start'],
                             'command': args, 'snapshot_monotonic_ns': current['monotonic_ns']}
    require(found is not None, 'missing direct-child round-three process attestation')
    return found


def scheduler_prefix(path, machine, queue, start, end, host):
    """Read every row; validate every prefix row through the first upper bracket."""
    prefix, outside, bracketed, previous = [], [], False, None
    for line, row in enumerate(host.rows(path), 1):
        if bracketed and row.get('kind') == 'error':
            # Old observer errors have no clock. Their position AFTER a completed
            # upper-bracketing census establishes exclusion; invent no timestamp.
            outside.append({'line': line, 'observation': row})
            continue
        require(row.get('kind') != 'error', 'scheduler observer error inside certified prefix')
        now = row.get('monotonic_ns')
        require(type(now) is int and (previous is None or now > previous),
                'invalid scheduler observation order')
        previous = now
        if bracketed:
            continue
        if prefix:
            require(0 < now - prefix[-1]['monotonic_ns'] <= 75_000_000_000,
                    'scheduler observation gap inside certified prefix')
        else:
            require(row.get('stage') == 'before' and now <= start, 'missing scheduler lower bracket')
        host.check_scheduler_snapshot(row, machine, queue)
        prefix.append(row)
        bracketed = now >= end
    require(bracketed and len(prefix) >= 2, 'missing scheduler upper bracket')
    return {'first_line': 1, 'last_line': len(prefix), 'observations': len(prefix),
            'started_monotonic_ns': prefix[0]['monotonic_ns'],
            'completed_monotonic_ns': prefix[-1]['monotonic_ns'],
            'maximum_gap_ns': max(b['monotonic_ns'] - a['monotonic_ns'] for a, b in zip(prefix, prefix[1:]))}, outside


def _certify(plan, proof, collection, raw, old, host):
    routing = old.routing
    spec = old.validate_plan(plan)
    sources = source_semantics(plan, old, host)
    require(collection.get('kind') == old.KIND + '_collection' and collection.get('status') == 'complete'
            and collection.get('plan_sha256') == routing.identity(plan)
            and collection.get('prerequisites_sha256') == routing.identity(proof)
            and len(collection.get('rounds', [])) == 3, 'complete original collection required')
    expected = [str((Path(plan['output_root']) / f'collection/round-{i}/records.json').resolve()) for i in (1, 2, 3)]
    require([str(Path(ref['path']).resolve()) for ref in collection['rounds']] == expected,
            'original three round destinations changed')
    require(collection['host']['slots'] == host.SLOTS and collection['host']['machine'] == 'x86_64',
            'wrong original physical allocation')
    old.checked_wrappers(plan, spec, proof)
    require(proof['host']['machine'] == collection['host']['machine'], 'prerequisite architecture changed')
    original = routing.read_json(old.checked(proof['records']))
    old.validate_prerequisite_identity(plan, original)
    outcomes = old.prerequisite_status(spec, original)
    exact = old.expected_fingerprints(proof, original, outcomes)
    records = [routing.read_json(old.checked(ref)) for ref in collection['rounds']]
    for record in records:
        old.round_data(plan, spec, record, outcomes, exact)
    require(set(raw) == old.EVIDENCE, 'complete original raw evidence required')
    paths = {key: old.checked(ref) for key, ref in raw.items()}
    require(all(path.stat().st_size for path in paths.values()), 'empty raw evidence')
    for key in ('job_environment', 'parallel_environment', 'resource_quota'):
        host.stable_pair(paths[key])
    pairs = {key: host.boundary_pair(paths[key]) for key in
             ('job_environment', 'queue_configuration', 'parallel_environment', 'resource_quota', 'affinity')}
    queues = [host.check_configuration({key: values[i] for key, values in pairs.items()},
                                     collection['host'], collection['collection_pid']) for i in (0, 1)]
    require(queues[0] == queues[1], 'granted queue changed')
    require(host.accounting(json.loads(paths['accounting'].read_text()), collection)
            == {'failed': 0, 'exit_status': 0}, 'original accounting failed')
    require(host.process_facts(paths['process_observations'], plan, collection) == [],
            'original process coverage incomplete or competing compute observed')
    bounds = {'elapsed_round3': elapsed_bound(collection, records[2]),
              'process_round3': process_bound(plan, collection, paths['process_observations'], host)}
    start = collection['started_monotonic_ns']
    ends = [bound['completed_monotonic_ns'] for bound in bounds.values()]
    end = min(ends)
    # Validate coverage through BOTH independent upper bounds; certify the tighter.
    scheduler, errors = scheduler_prefix(paths['scheduler'], collection['host'], queues[0], start, max(ends), host)
    try:
        host.check_scheduler(paths['scheduler'], collection['host'], queues[0], collection)
    except (ValueError, KeyError, TypeError) as exc:
        original_failure = str(exc)
        original_failure_type = type(exc).__name__
    else:
        raise ValueError('original full scheduler protocol did not fail; no failed-collection salvage certificate')
    return {'kind': KIND, 'schema': 1, 'status': 'verified',
            'parent_sha256': routing.identity(plan), 'proof_sha256': routing.identity(proof),
            'collection_sha256': routing.identity(collection), 'collection': routing.seal(collection),
            'rounds': collection['rounds'][:2], 'raw_evidence': raw, 'source_semantics': sources,
            'window': {'started_monotonic_ns': start, 'completed_monotonic_ns': end},
            'bounds': bounds, 'scheduler_prefix': scheduler, 'outside_window_errors': errors,
            'original_protocol_status': 'failed', 'original_protocol_error': original_failure,
            'original_protocol_error_type': original_failure_type,
            'scientific_validation': 'all three original rounds, outcomes and exact fingerprints',
            'process_validation': 'complete original collection; no observed competing compute',
            'scope': 'whole sequential rounds 1 and 2 only; descriptive discovery evidence', 'promotion': False}


def certify(parent_plan, proof, collection, raw_evidence):
    """Replay with original sources in a fresh interpreter; never rebind the parent."""
    generator = checked(parent_plan['receipt_generator'])
    require(parent_plan['frozen_files'].get(str(generator)) == parent_plan['receipt_generator']['sha256'],
            'original receipt generator is not frozen')
    result = subprocess.run([sys.executable, str(Path(__file__).resolve()), '_replay'],
                            input=json.dumps({'parent_plan': parent_plan, 'proof': proof,
                                              'collection': collection, 'raw_evidence': raw_evidence}, allow_nan=False),
                            text=True, capture_output=True, timeout=REPLAY_TIMEOUT_S, check=False)
    require(result.returncode == 0, 'original-source window replay failed: ' + result.stderr[-2000:])
    value = json.loads(result.stdout)
    require(value.get('kind') == KIND and value.get('status') == 'verified', 'invalid window replay response')
    return value


def validate(parent_plan, proof, certificate):
    import routing
    expected = certify(parent_plan, proof, routing.unseal(certificate['collection']), certificate['raw_evidence'])
    require(expected == certificate, 'window certificate differs from original evidence replay')
    return {'schema': 1, 'status': 'verified', 'certificate_sha256': routing.identity(certificate)}


def original_modules(plan):
    # The fresh worker has imported only stdlib. Put the old bench directory first
    # BEFORE importing any scientific/host module: no current module can leak in.
    directory = checked(plan['receipt_generator']).parent
    for name, expected in PINNED_SOURCES.items():
        path = directory / name
        require(hashlib.sha256(path.read_bytes()).hexdigest() == expected
                and plan['frozen_files'].get(str(path)) == expected,
                'window clock proof requires the reviewed original source')
    sys.path.insert(0, str(directory))
    old = importlib.import_module('routing_discovery')
    host = importlib.import_module('scc_routing_host')
    require(Path(old.__file__).resolve().parent == directory
            and Path(host.__file__).resolve().parent == directory, 'original validator directory changed')
    return old, host


def main():
    if sys.argv[1:] == ['_replay']:
        args = json.load(sys.stdin)
        old, host = original_modules(args['parent_plan'])
        value = _certify(args['parent_plan'], args['proof'], args['collection'], args['raw_evidence'], old, host)
        print(json.dumps(value, allow_nan=False))
        return 0
    if sys.argv[1:] == ['validate-stdin']:
        args = json.load(sys.stdin)
        print(json.dumps(validate(args['parent_plan'], args['proof'], args['certificate']), allow_nan=False))
        return 0
    import routing_discovery as discovery
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--parent-plan', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--collection', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    plan, proof, collection = map(discovery.load, (args.parent_plan, args.proof, args.collection))
    root = Path(plan['output_root']) / 'isolation'
    raw = {key: discovery.reference(root / (key + ('.json' if key == 'accounting' else '.jsonl')))
           for key in discovery.EVIDENCE}
    discovery.write(args.output, certify(plan, proof, collection, raw))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
