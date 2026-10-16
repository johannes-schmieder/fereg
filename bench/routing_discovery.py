#!/usr/bin/env python3
"""Prospective Linux/SCC routing screen, separate from legacy A/B/C selection.

Freeze on the deployment host, run fresh Linux prerequisites, then collect exactly
three unchanged routing_run A rounds. Each round keeps the producer's one warmup,
three repetitions and fixed order. No timing-dependent continuation is permitted.
Prerequisites have a separate four-hour producer budget; collection has three
four-hour producer budgets. Submit collection with an external twelve-hour job
limit as well; interruption cannot qualify a partial collection. Any new failure
stops collection without retry. Aggregate
only after external reservation/process/accounting evidence is available. The
nine paired slots are descriptive point estimates, not independent samples or a
confidence claim. This tool never reads B/C case files or promotes a policy.

The separate prospective parallel schema 3 retains the full parent here and
derives two exact family subsets for execution. Only its six-allocation replay
can score the full parent; a subset is not an independently valid tournament.

The isolation descriptor is produced by a separately reviewed, frozen collector.
Its raw scheduler/configuration/affinity/process/accounting references are kept;
hash binding establishes provenance, not independent proof of their truth. The
reservation facts and observed process noise are deliberately separate fields.
"""
from __future__ import annotations

import argparse
import copy
import json
import math
import os
from pathlib import Path
import platform
import socket
import statistics
import subprocess
import sys
import time

import routing
import routing_run
from time_exact import sha256_file as digest

KIND = 'routing_scc_discovery'
PORTFOLIO_KIND = 'routing_scc_portfolio_discovery'
GRID = ['baseline', *[f'h{h}-k{k}' for k in routing.TREES for h in routing.CHECKPOINTS]]
DESIGN = {'rounds': 3, 'measured_slots': 9, 'warmups_per_round': 1,
          'per_call_timeout_s': 120.0, 'prerequisite_budget_s': 14400.0,
          'per_round_budget_s': 14400.0, 'collection_budget_s': 43200.0,
          'order': 'unchanged producer order, repeated three times',
          'stopping': 'any new failure stops all remaining rounds; no retries',
          'inference': 'point estimates only; slots are not asserted independent'}
EVIDENCE = {'scheduler', 'queue_configuration', 'parallel_environment',
            'resource_quota', 'job_environment', 'affinity', 'process_observations', 'accounting'}
LIBRARY_LIMITS = {k: '1' for k in
                  ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'NUMEXPR_NUM_THREADS']}
SCHEDULER_QUERY_POLICY = {
    'kind': 'qstat-pair-retry-v1', 'maximum_pair_attempts': 2,
    'query_budget_s': 20, 'command_budget_s': 10, 'successful_census_gap_s': 75,
    'cadence_s': 60, 'retryable': ['transport', 'nonzero', 'timeout'],
    'semantic_failure': 'terminal', 'producer_retries': 0, 'replacements': 0,
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def reference(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': digest(path)}


def checked(ref):
    require(set(ref) == {'path', 'sha256'} and routing.sha(ref['sha256']), 'invalid file binding')
    path = Path(ref['path']).resolve(strict=True)
    require(digest(path) == ref['sha256'], 'frozen file changed: ' + str(path))
    return path


def write(path, value):
    with Path(path).open('x') as stream:
        stream.write(json.dumps(routing.seal(value), indent=2, allow_nan=False) + '\n')


def load(path):
    return routing.unseal(routing.read_json(Path(path)))


def validate_spec(spec, *, portfolio=False, candidate=None):
    routing.validate_manifest(spec)
    require(spec.get('experiment_kind', 'routing') == ('portfolio' if portfolio else 'routing')
            and spec.get('purpose') != 'ablation_only', 'discovery requires the routing tournament')
    require(spec['candidates'] == (['baseline', candidate or routing.PORTFOLIO_V1] if portfolio else GRID)
            and len(spec['panels']['A']) == 42, 'discovery candidate portfolio or 42 A cells differs')


def partitions(spec):
    """A prospective, structural split; never consult times or availability."""
    import routing_cases
    validate_spec(spec, portfolio=True, candidate=routing.PORTFOLIO_UNSEEDED)
    cells = spec['panels']['A']
    families = sorted({cell['family'] for cell in cells})
    require(set(families) == routing.PORTFOLIO_COVERAGE and len(families) == 21
            and all(sorted(c['threads'] for c in cells if c['family'] == family) == [1, 4]
                                      for family in families), 'partition requires 21 complete worker pairs')
    for family in families:
        pair = [c for c in cells if c['family'] == family]
        require(all(type(c.get('absorbed_terms')) is int and type(c.get('categorical_intercepts')) is bool
                    and c['comparators'] == routing_cases.comparators(c, sampled=True, compact_it=True) for c in pair)
                and all(pair[0].get(k) == pair[1].get(k) for k in
                        ('case_sha256', 'body_sha256', 'absorbed_terms', 'categorical_intercepts',
                         'comparators', 'memory_limit_bytes')),
                'worker pair or complete mathematical comparator frontier changed')
    groups = [families[:10], families[10:]]
    slots = [sum(len(set(spec['candidates'] + spec['protections'] + c['comparators']))
                 for c in cells if c['family'] in group) for group in groups]
    require(sum(slots) == 360, 'partition must retain the complete 360-slot comparator frontier')
    budgets = [14400 * slots[0] // 360]
    budgets.append(14400 - budgets[0])
    return [{'shard': index + 1, 'parent_manifest_sha256': routing.identity(spec),
             'families': group, 'cell_ids': [c['id'] for c in cells if c['family'] in group],
             'arm_slots': slots[index], 'inclusive_allocation_budget_s': budgets[index],
             'maximum_native_calls': 7 * slots[index]}
            for index, group in enumerate(groups)]


def partition_spec(spec, partition):
    """Derive an execution view only after validating the complete parent."""
    require(isinstance(partition, dict) and type(partition.get('shard')) is int
            and partition['shard'] in (1, 2), 'invalid discovery partition')
    expected = partitions(spec)[partition['shard'] - 1]
    require(routing.identity(partition) == routing.identity(expected), 'discovery partition differs from full parent')
    result = copy.deepcopy(spec)
    result['panels']['A'] = [c for c in result['panels']['A'] if c['id'] in expected['cell_ids']]
    result['failure_policy']['aggregate_budget_s'] = expected['inclusive_allocation_budget_s']
    return result


def execution_spec(plan):
    spec = routing.unseal(plan['tournament'])
    if 'discovery_partition' not in plan:
        return spec
    contract = plan.get('round_contract', {})
    part = plan['discovery_partition']
    require(plan.get('kind') == PORTFOLIO_KIND + '_plan'
            and routing.identity(plan.get('scheduler_query_policy')) == routing.identity(SCHEDULER_QUERY_POLICY)
            and set(contract) == {'parallel_plan_sha256', 'logical_round', 'shard', 'attempt', 'partition_sha256'}
            and routing.sha(contract['parallel_plan_sha256'])
            and type(contract['logical_round']) is int and contract['logical_round'] in (1, 2, 3)
            and type(contract['attempt']) is int and contract['attempt'] == 1
            and type(contract['shard']) is int and contract['shard'] == part.get('shard')
            and contract['partition_sha256'] == routing.identity(part), 'unbound discovery partition')
    return partition_spec(spec, part)


def execution_budget(plan):
    return execution_spec(plan)['failure_policy']['aggregate_budget_s'] if 'discovery_partition' in plan else 14400.0


def validate_partition_records(plan, records):
    if 'discovery_partition' in plan:
        require(records.get('discovery_partition') == plan['discovery_partition']
                and records.get('parent_manifest_sha256') == routing.identity(routing.unseal(plan['tournament'])),
                'records lost full-parent/subset binding')
    else:
        require('discovery_partition' not in records and 'parent_manifest_sha256' not in records,
                'unexpected partition records')


def freeze(config):
    """Bind deployment paths. Config contains manifest/harness/plugins/cases/witnesses,
    source_checks (nonempty file list), and a reviewed receipt_generator path.
    Only A input paths may be supplied. Witness closures and A sidecars are frozen.
    """
    spec = load(config['manifest'])
    portfolio = spec.get('experiment_kind') == 'portfolio'
    validate_spec(spec, portfolio=portfolio, candidate=routing.portfolio_candidate(spec) if portfolio else None)
    cells = spec['panels']['A']
    arms = set(spec['candidates'] + spec['protections'] + [a for c in cells for a in c['comparators']])
    require(set(config['plugins']) == arms, 'artifact coverage changed')
    require(set(config['cases']) == set(config['witnesses']) == {c['id'] for c in cells},
            'only complete A input coverage is permitted')
    require(config.get('source_checks'), 'freeze completed source-check evidence')
    paths = {str(p.resolve()): h for p, h in routing_run.tool_identities().items()}
    for path in [__file__, Path(__file__).parent / 'tests/test_routing_discovery.py',
                 Path(__file__).parent / 'block_host.py',
                 config['manifest'], config['harness'], config['receipt_generator'], *config['source_checks']]:
        ref = reference(path)
        paths[ref['path']] = ref['sha256']
    require(digest(Path(config['harness'])) == spec['harness_sha256'], 'harness identity changed')
    for arm, name in config['plugins'].items():
        path = Path(name).resolve(strict=True)
        require(digest(path) == spec['arms'][arm]['sha256'], 'artifact identity changed')
        with path.open('rb') as stream:
            require(stream.read(4) == b'\x7fELF', 'discovery requires Linux ELF artifacts')
        paths[str(path)] = digest(path)
    for cell in cells:
        cid = cell['id']
        case = Path(config['cases'][cid]).resolve(strict=True)
        require(digest(case) == cell['case_sha256'], 'A input identity changed')
        paths[str(case)] = digest(case)
        sidecar = case.with_suffix('.json')
        meta = routing.read_json(sidecar)
        paths[str(sidecar)] = digest(sidecar)
        if 'replay_reference' in meta:
            ref = meta['replay_reference']
            source = checked({'path': str(sidecar.parent / ref['file']), 'sha256': ref['sha256']})
            paths[str(source)] = ref['sha256']
        witness = Path(config['witnesses'][cid]).resolve(strict=True)
        value, bound = routing_run.evidence(witness, cell, spec,
            list(dict.fromkeys(spec['candidates'] + spec['protections'] + cell['comparators'])),
            prerequisites_only=True, local_oracle='oracle' in meta or 'replay_reference' in meta
            or cell['family'] == 'inactive-columns')
        paths.update({str(p): h for p, h in bound.items()})
        if 'independent_witness' in value:
            source = witness.parent / value['independent_witness']['path']
            descriptor = routing.read_json(source) if source.suffix == '.json' else {}
            if descriptor.get('kind') == routing_run.routing_oracle.KIND:
                _, bound = routing_run.routing_oracle.load_reference(source, cell['case_sha256'], cell['body_sha256'])
                paths.update({str(p): h for p, h in bound.items()})
    root = routing.scratch(Path(config['output_root']))
    require(not root.exists(), 'freeze a fresh campaign output root')
    return {'kind': (PORTFOLIO_KIND if portfolio else KIND) + '_plan', 'schema': 1, 'design': DESIGN, 'tournament': routing.seal(spec),
            'output_root': str(root), 'library_limits': LIBRARY_LIMITS,
            'harness': str(Path(config['harness']).resolve()),
            **{key: {k: str(Path(v).resolve()) for k, v in config[key].items()}
               for key in ['plugins', 'cases', 'witnesses']},
            'receipt_generator': reference(config['receipt_generator']),
            'source_checks': [reference(p) for p in config['source_checks']],
            'frozen_files': paths,
            'producer_tools': {p.name: h for p, h in routing_run.tool_identities().items()}}


def validate_plan(plan):
    require(plan.get('kind') in {KIND + '_plan', PORTFOLIO_KIND + '_plan'} and plan.get('design') == DESIGN
            and plan.get('library_limits') == LIBRARY_LIMITS, 'wrong discovery plan')
    scheduler_query_policy(plan)
    spec = routing.unseal(plan['tournament'])
    portfolio = plan['kind'] == PORTFOLIO_KIND + '_plan'
    validate_spec(spec, portfolio=portfolio, candidate=routing.portfolio_candidate(spec) if portfolio else None)
    for path, sha in plan['frozen_files'].items():
        checked({'path': path, 'sha256': sha})
    require(plan['producer_tools'] == {p.name: h for p, h in routing_run.tool_identities().items()},
            'producer/checker source changed')
    return execution_spec(plan)


def scheduler_query_policy(plan):
    """Only a prospective portfolio round view may enable query recovery."""
    if 'scheduler_query_policy' not in plan:
        return None
    contract = plan.get('round_contract', {})
    require(plan.get('kind') == PORTFOLIO_KIND + '_plan'
            and routing.identity(plan['scheduler_query_policy']) == routing.identity(SCHEDULER_QUERY_POLICY)
            and isinstance(contract, dict)
            and set(contract) == ({'parallel_plan_sha256', 'logical_round', 'shard', 'attempt', 'partition_sha256'}
                                  if 'discovery_partition' in plan else {'parallel_plan_sha256', 'logical_round', 'attempt'})
            and routing.sha(contract.get('parallel_plan_sha256'))
            and type(contract.get('logical_round')) is int and 1 <= contract['logical_round'] <= 3
            and type(contract.get('attempt')) is int and contract['attempt'] == 1,
            'scheduler query policy requires its frozen portfolio round')
    if 'discovery_partition' in plan:
        execution_spec(plan)
    return plan['scheduler_query_policy']


def host():
    require(platform.system() == 'Linux', 'fresh discovery evidence must be produced on Linux')
    require(os.environ.get('JOB_ID') and os.environ.get('NSLOTS'), 'run inside the reserved SCC job')
    return {'hostname': socket.gethostname(), 'job_id': os.environ['JOB_ID'],
            'slots': int(os.environ['NSLOTS']), 'affinity': sorted(os.sched_getaffinity(0)),
            'machine': platform.machine(),
            'library_limits': {k: os.environ.get(k) for k in
                ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'NUMEXPR_NUM_THREADS']}}


def invoke(plan, spec, witnesses, output, prerequisite=False):
    ids = {c['id'] for c in spec['panels']['A']}
    arms = set(spec['candidates'] + spec['protections'] + [a for c in spec['panels']['A'] for a in c['comparators']])
    options = {'discovery_partition': plan['discovery_partition']} if 'discovery_partition' in plan else {}
    return routing_run.run(routing.unseal(plan['tournament']), 'A', None, Path(plan['harness']),
        {k: Path(v) for k, v in plan['plugins'].items() if k in arms},
        {k: Path(v) for k, v in plan['cases'].items() if k in ids},
        {k: v for k, v in witnesses.items() if k in ids}, output,
        timeout=120.0, budget=execution_budget(plan), prerequisites_only=prerequisite, **options)


def prerequisite_status(spec, records):
    require(records.get('status') in {'complete', 'complete_with_failed_cells'}, 'incomplete prerequisites')
    outcomes = {}
    for cell in spec['panels']['A']:
        cid = cell['id']
        arms = list(dict.fromkeys(spec['candidates'] + spec['protections'] + cell['comparators']))
        result = routing_run.prerequisite_outcomes(spec, arms,
            [r for r in records['prerequisites'] if r['cell'] == cid],
            [r for r in records['records'] if r['cell'] == cid])
        for arm, outcome in result.items():
            require(outcome['status'] == 'passed' or
                    (routing.arm_role(spec, arm) == 'optional_comparator' and outcome['status'] == 'unavailable'),
                    'prerequisite failure blocks discovery: ' + cid + '/' + arm)
            outcomes[cid, arm] = outcome
    return outcomes


def prerequisites(plan, output):
    spec, machine = validate_plan(plan), host()
    require(machine['library_limits'] == plan['library_limits'], 'set all frozen library thread limits before launch')
    require(output == Path(plan['output_root']) / 'prerequisites', 'prerequisite destination is frozen')
    output.mkdir(parents=True, exist_ok=False)
    result = invoke(plan, spec, {k: Path(v) for k, v in plan['witnesses'].items()}, output / 'producer', True)
    status, error = 'ready', None
    try:
        prerequisite_status(spec, result)
        require(set(result['qualified_wrappers']) == {c['id'] for c in spec['panels']['A']}, 'missing Linux wrappers')
        validate_plan(plan)
    except ValueError as exc:
        status, error = 'incomplete', str(exc)
    value = {'kind': KIND + '_prerequisites', 'plan_sha256': routing.identity(plan),
             'status': status, 'error': error, 'host': machine,
             'records': reference(output / 'producer/records.json'),
             'wrappers': {k: {'path': str(output / 'producer' / v['path']), 'sha256': v['sha256']}
                          for k, v in result['qualified_wrappers'].items()}}
    write(output / 'prerequisites.json', value)
    return value


def fingerprints(records, outcomes):
    found = {}
    for row in [*records['prerequisites'], *records['records']]:
        if row['status'] == 'passed' and outcomes[row['cell'], row['arm']]['status'] == 'passed':
            key = row['cell'], row['arm']
            value = tuple(row.get(k) for k in ['science_sha256', 'witness_sha256', 'prepared_sha256'])
            require(all(routing.sha(v) for v in value), 'missing exact fingerprint')
            require(key not in found or found[key] == value, 'exact repeat changed')
            found[key] = value
    return found


def round_data(plan, spec, records, expected_outcomes, expected_fingerprints):
    validate_partition_records(plan, records)
    require(records.get('platform', '').startswith('Linux') and records.get('tool_sha256') == plan['producer_tools'],
            'round is not from the frozen Linux producer')
    require(records.get('purpose') == 'policy_tournament' and records.get('warmups') == 1
            and records.get('repetitions') == 3 and records.get('timeout_s') == 120.0
            and records.get('budget_s') == execution_budget(plan), 'round measurement contract changed')
    times, outcomes, _ = routing.observations(spec, 'A', spec['candidates'], records, None)
    require({k: v['status'] for k, v in outcomes.items()}
            == {k: v['status'] for k, v in expected_outcomes.items()},
            'new outcome stops discovery without retry')
    for row in records['prerequisites']:
        key = row['cell'], row['arm']
        if outcomes[key]['status'] == 'unavailable':
            origin = {k: v for k, v in row.items() if k not in {'observation_source', 'prior_proof'}}
            require(row.get('observation_source') == 'prior_prerequisite'
                    and routing.observation_identity(origin) in expected_outcomes[key]['failure_sha256'],
                    'optional unavailability must carry the original Linux failure, never retry')
    require(fingerprints(records, outcomes) == expected_fingerprints, 'cross-round exact fingerprint changed')
    return times


def expected_fingerprints(proof, original, outcomes):
    # The science prerequisite uses ready wrappers; timing uses newly checked
    # wrappers. Bind the deliberate wrapper transition, then require exact repeats.
    return {key: (value[0], proof['wrappers'][key[0]]['sha256'], value[2])
            for key, value in fingerprints(original, outcomes).items()}


def checked_wrappers(plan, spec, proof):
    require(proof.get('kind') == KIND + '_prerequisites' and proof.get('status') == 'ready'
            and proof.get('plan_sha256') == routing.identity(plan), 'fresh Linux prerequisites required')
    require(set(proof['wrappers']) == {c['id'] for c in spec['panels']['A']}, 'complete checked wrappers required')
    witnesses = {k: checked(v) for k, v in proof['wrappers'].items()}
    for cell in spec['panels']['A']:
        routing_run.evidence(witnesses[cell['id']], cell, spec,
            list(dict.fromkeys(spec['candidates'] + spec['protections'] + cell['comparators'])))
    return witnesses


def collect(plan, proof, output):
    require(plan.get('kind') == KIND + '_plan', 'portfolio collection requires the closed parallel protocol')
    spec, machine = validate_plan(plan), host()
    require(machine['library_limits'] == plan['library_limits'], 'set all frozen library thread limits before launch')
    require(output == Path(plan['output_root']) / 'collection', 'collection destination is frozen')
    require(proof.get('kind') == KIND + '_prerequisites' and proof.get('status') == 'ready'
            and proof.get('plan_sha256') == routing.identity(plan), 'fresh Linux prerequisites required')
    require(proof['host']['machine'] == machine['machine'], 'prerequisite architecture changed')
    original = routing.read_json(checked(proof['records']))
    validate_prerequisite_identity(plan, original)
    expected = prerequisite_status(spec, original)
    exact = expected_fingerprints(proof, original, expected)
    witnesses = checked_wrappers(plan, spec, proof)
    output.mkdir(parents=True, exist_ok=False)
    result = {'kind': KIND + '_collection', 'plan_sha256': routing.identity(plan),
              'prerequisites_sha256': routing.identity(proof), 'host': machine, 'rounds': [], 'status': 'incomplete',
              'collection_pid': os.getpid(), 'started_wall_time_ns': time.time_ns(),
              'started_monotonic_ns': time.monotonic_ns()}
    try:
        for index in range(3):
            require(host() == machine, 'allocation, affinity or library environment changed')
            validate_plan(plan)
            directory = output / f'round-{index + 1}'
            try:
                record = invoke(plan, spec, witnesses, directory)
            finally:
                if (directory / 'records.json').exists():
                    result['rounds'].append(reference(directory / 'records.json'))
            round_data(plan, spec, record, expected, exact)
        validate_plan(plan)
        require(host() == machine, 'host changed at collection end')
        result['status'] = 'complete'
    except (ValueError, OSError, RuntimeError) as exc:
        result['error'] = str(exc)
    finally:
        result.update(completed_wall_time_ns=time.time_ns(), completed_monotonic_ns=time.monotonic_ns())
        write(output / 'collection.json', result)
    return result


def isolation(plan, collection, receipt):
    require(receipt.get('kind') == KIND + '_isolation' and receipt.get('plan_sha256') == routing.identity(plan)
            and receipt.get('collection_sha256') == routing.identity(collection), 'isolation binding changed')
    require(receipt.get('generator') == plan['receipt_generator'], 'unreviewed isolation generator')
    checked(receipt['generator'])
    require(receipt.get('host') == collection['host'] and receipt.get('rounds') == collection['rounds'],
            'isolation does not cover this allocation and all rounds')
    require(set(receipt.get('raw_evidence', {})) == EVIDENCE, 'missing external isolation evidence')
    for ref in receipt['raw_evidence'].values():
        require(checked(ref).stat().st_size > 0, 'empty external isolation evidence')
    facts = receipt['reservation']
    require(facts.get('num_proc') == facts.get('slots') == collection['host']['slots']
            and facts['slots'] >= max(c['threads'] for c in routing.unseal(plan['tournament'])['panels']['A'])
            and facts.get('pe_allocation_rule') == '$pe_slots'
            and facts.get('rqs_enabled') is True and facts.get('rqs_all_queues') is True
            and facts.get('rqs_limit') == 'num_proc' and facts.get('single_host') is True,
            'scheduler evidence does not establish a complete host reservation')
    require(receipt.get('accounting') == {'failed': 0, 'exit_status': 0}, 'scheduler job did not complete')
    require(receipt.get('observed_compute_overlap') == [] and receipt.get('observation_complete') is True,
            'competing compute or incomplete isolation observation blocks discovery')
    # The reviewed collector rederives facts from captured external evidence. A
    # zero exit or descriptor assertions alone do not establish reservation proof.
    # The fixed five-minute bound applies only to pure offline evidence replay.
    result = subprocess.run([sys.executable, receipt['generator']['path'], 'validate-stdin'],
        input=json.dumps({'plan': plan, 'collection': collection, 'receipt': receipt}, allow_nan=False),
        text=True, capture_output=True, timeout=300, check=False)
    expected = {'schema': 1, 'status': 'verified', 'plan_sha256': routing.identity(plan),
                'collection_sha256': routing.identity(collection), 'receipt_sha256': routing.identity(receipt)}
    require(result.returncode == 0 and len(result.stdout) <= 4096,
            'external isolation replay validator failed')
    require(json.loads(result.stdout) == expected, 'isolation replay did not verify these exact observations')


def score(spec, times, outcomes):
    """Legacy A point-estimate criterion for arbitrary complete paired-slot vectors.
    Deliberately excludes bootstrap machinery and all B/C selection semantics.
    """
    ranking = []
    for arm in spec['candidates']:
        violations, worst, ratios, comparisons = 0, None, [], []
        for cell in spec['panels']['A']:
            cid, t = cell['id'], times[cell['id'], arm]
            ratios.append(statistics.median(c / b for c, b in zip(t, times[cid, 'baseline'])))
            for ref in dict.fromkeys(spec['protections'] + cell['comparators']):
                protected, base = ref in spec['protections'], times[cid, ref]
                detail = {'cell': cid, 'reference': ref, 'protection': protected}
                if base is None:
                    require(not protected and outcomes[cid, ref]['status'] == 'unavailable', 'unresolved reference')
                    comparisons.append({**detail, 'status': 'availability_dominance', 'runtime_identified': False})
                    continue
                require(len(base) == len(t) and len(t) > 0, 'unpaired observations')
                ratio = statistics.median(c / b for c, b in zip(t, base))
                allowance = 1 + max(.05 if protected else .20, .001 / statistics.median(base))
                normalized = (ratio - 1) / (allowance - 1)
                violations += int(protected and normalized > 1)
                if not protected:
                    worst = normalized if worst is None else max(worst, normalized)
                comparisons.append({**detail, 'status': 'compared', 'median_ratio': ratio,
                                    'upper_allowance': allowance, 'pass': normalized <= 1})
        ranking.append({'arm': arm, 'baseline_violations': violations,
                        'worst_internal_normalized': worst,
                        'geomean_baseline_ratio': math.exp(statistics.mean(map(math.log, ratios))),
                        'comparisons': comparisons})
    ranking.sort(key=lambda r: (r['baseline_violations'], math.inf if r['worst_internal_normalized'] is None
                               else r['worst_internal_normalized'], r['geomean_baseline_ratio'], routing.conservative(r['arm'])))
    return ranking


def aggregate(plan, proof, collection, receipt):
    require(plan.get('kind') == KIND + '_plan', 'portfolio aggregation requires the closed parallel protocol')
    spec = validate_plan(plan)
    require(collection.get('kind') == KIND + '_collection' and collection.get('status') == 'complete'
            and collection.get('plan_sha256') == routing.identity(plan)
            and collection.get('prerequisites_sha256') == routing.identity(proof)
            and len(collection.get('rounds', [])) == 3, 'incomplete discovery cannot select')
    isolation(plan, collection, receipt)
    require([str(Path(ref['path']).resolve()) for ref in collection['rounds']]
            == [str((Path(plan['output_root']) / f'collection/round-{i}/records.json').resolve()) for i in range(1, 4)],
            'rounds must be the three distinct prospectively frozen outputs')
    checked_wrappers(plan, spec, proof)
    original = routing.read_json(checked(proof['records']))
    validate_prerequisite_identity(plan, original)
    outcomes = prerequisite_status(spec, original)
    exact = expected_fingerprints(proof, original, outcomes)
    combined = {}
    for ref in collection['rounds']:
        data = round_data(plan, spec, routing.read_json(checked(ref)), outcomes, exact)
        for key, values in data.items():
            if values is None:
                combined[key] = None
            else:
                combined.setdefault(key, []).extend(values)
    require(all(v is None or len(v) == 9 for v in combined.values()), 'nine measured paired slots required')
    ranking = score(spec, combined, outcomes)
    return {'kind': KIND + '_shortlist', 'schema': 1, 'plan_sha256': routing.identity(plan),
            'collection_sha256': routing.identity(collection), 'isolation_sha256': routing.identity(receipt),
            'status': 'discovery_only', 'claim': DESIGN['inference'], 'promotion': False,
            'ranking': ranking, 'shortlist': [r['arm'] for r in ranking[:4]]}


def validate_prerequisite_identity(plan, records):
    spec = execution_spec(plan)
    validate_partition_records(plan, records)
    arms = set(spec['candidates'] + spec['protections'] + [a for c in spec['panels']['A'] for a in c['comparators']])
    require(records.get('platform', '').startswith('Linux')
            and records.get('purpose') == 'scientific_prerequisites'
            and records.get('stage') == 'A' and records.get('selection_sha256') is None
            and records.get('repetitions') == 1 and records.get('warmups') == 0
            and records.get('timeout_s') == 120.0 and records.get('budget_s') == execution_budget(plan)
            and records.get('manifest_sha256') == routing.identity(spec)
            and records.get('tool_sha256') == plan['producer_tools']
            and records.get('harness_sha256') == spec['harness_sha256']
            and records.get('artifacts') == {a: spec['arms'][a]['sha256'] for a in arms}
            and records.get('cases') == {c['id']: c['case_sha256'] for c in spec['panels']['A']},
            'fresh Linux prerequisite identities required; Mac evidence cannot carry')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['freeze', 'prerequisites', 'collect', 'aggregate'])
    parser.add_argument('--plan', type=Path, required=True, help='config JSON for freeze; sealed plan otherwise')
    parser.add_argument('--proof', type=Path)
    parser.add_argument('--collection', type=Path)
    parser.add_argument('--isolation', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = routing.scratch(args.output)
    if args.command == 'freeze':
        write(output, freeze(routing.read_json(args.plan)))
        return 0
    plan = load(args.plan)
    if args.command == 'prerequisites':
        result = prerequisites(plan, output)
    elif args.command == 'collect':
        result = collect(plan, load(args.proof), output)
    else:
        result = aggregate(plan, load(args.proof), load(args.collection), load(args.isolation))
        write(output, result)
    return int(result['status'] == 'incomplete')


if __name__ == '__main__':
    raise SystemExit(main())
