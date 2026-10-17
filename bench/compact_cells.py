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
FACTOR_KIND = 'compact_cells_factor_study_v1'
FACTOR_ARMS = [*ARMS, 'cells_v2_df']
FACTOR_LIMITS = {**LIMITS, 'seed': 2026101604, 'data_seed': LIMITS['seed'],
                 'parent_control_repeats': 1, 'diagnostic_budget_s': 1800, 'diagnostic_repeats': 2,
                 'diagnostic_families': ['dense', 'repeated', 'hubs', 'high-degree-weak']}
DEFERRED_KIND = 'compact_cells_deferred_study_v1'
DEFERRED_ARMS = ['baseline', 'cells', 'cells_deferred']
DEFERRED_LIMITS = {**FACTOR_LIMITS, 'seed': 2026101701}
FOLLOWUP_KINDS = {FACTOR_KIND, DEFERRED_KIND}
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


def contract(plan):
    """Select a fixed protocol without changing the original campaign globals."""
    kind = plan.get('kind', KIND)
    require(kind in {KIND, *FOLLOWUP_KINDS}, 'unknown campaign kind')
    return {KIND: (ARMS, LIMITS), FACTOR_KIND: (FACTOR_ARMS, FACTOR_LIMITS),
            DEFERRED_KIND: (DEFERRED_ARMS, DEFERRED_LIMITS)}[kind]


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


def schedule(cell_ids, *, arms=None, limits=None):
    """Six complete blocks; each cell has a seeded Latin rotation over arms.

    The first len(arms) blocks balance every arm at every position; remaining
    blocks preserve a maximum position-count difference of one. Cell order is
    independently shuffled in every block. Outcomes cannot alter this order.
    """
    arms, limits = ARMS if arms is None else arms, LIMITS if limits is None else limits
    rng = random.Random(limits['seed'])
    initial = {}
    for cid in cell_ids:
        initial[cid] = rng.sample(arms, len(arms))
    blocks = []
    for block in range(limits['blocks']):
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


def validate_contract(plan):
    arms, limits = contract(plan)
    require(plan['limits'] == limits and plan['arms'] == arms
            and plan['families'] == FAMILIES and plan['library_limits'] == LIBRARY_LIMITS,
            'campaign contract changed')
    expected = [f'{f}-t{w}' for f in FAMILIES for w in limits['workers']]
    require([c['id'] for c in plan['cells']] == expected
            and plan['blocks'] == schedule(expected, arms=arms, limits=limits),
            'case coverage or frozen order changed')
    for cell in plan['cells']:
        inputs = set(arms) | set(ARMS) if plan['kind'] == DEFERRED_KIND else set(arms)
        require(cell['id'] == f"{cell['family']}-t{cell['threads']}"
                and set(cell['inputs']) == inputs, 'incomplete arm/cell coverage')


def parent_dependencies(parent):
    """Verify retained data/artifacts; historical tools remain bound by the parent.

    A follow-up necessarily changes its tools. Never rewrite the old plan or
    rebind its old tool hashes to the new producer closure.
    """
    require(parent['kind'] == KIND, 'follow-up study requires the original four-arm parent')
    validate_contract(parent)
    bindings = [parent[key] for key in ['generator', 'harness', 'plugin']]
    for cell in parent['cells']:
        meta_path = checked(cell['metadata'])
        meta = read(meta_path)
        source = meta_path.with_suffix('.case')
        bindings.extend([cell['metadata'], cell['witness'], *cell['inputs'].values(),
                         {'path': str(source), 'sha256': meta['case_sha256']}])
        witness, _ = routing_oracle.load_reference(checked(cell['witness']),
                                                  meta['case_sha256'], meta['body_sha256'])
        # The loader also returns CURRENT producer/parser paths. It validates
        # their bytes against the witness; tool_files binds those paths in the
        # new closure. Only witness artifacts belong to the parent closure.
        bindings.append(ref(witness['_arrays_path']))
        for item in cell['inputs'].values():
            require(routing_cases.inspect_case(checked(item))['body_sha256'] == meta['body_sha256'],
                    'parent request changed data/model body')
    result = {item['path']: item['sha256'] for item in bindings}
    for path, sha in result.items():
        require(parent['frozen_files'].get(path) == sha, 'unbound parent dependency')
        checked({'path': path, 'sha256': sha})
    return result


def prepare_factor(root, parent_path, harness, plugin):
    return prepare_followup(root, parent_path, harness, plugin, FACTOR_KIND, 'cells_v2_df')


def prepare_deferred(root, parent_path, harness, plugin):
    return prepare_followup(root, parent_path, harness, plugin, DEFERRED_KIND, 'cells_deferred')


def prepare_followup(root, parent_path, harness, plugin, kind, new_arm):
    arms, limits = contract({'kind': kind})
    parent_binding = ref(parent_path)
    parent = routing.unseal(read(parent_path))
    dependencies = parent_dependencies(parent)
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=False)
    frozen = [ref(path) for path in [harness, plugin, *tool_files()]]
    cells_out = []
    for cell in parent['cells']:
        target = root / f"{cell['id']}-{new_arm}.case"
        routing.prepare(checked(cell['inputs']['baseline']), target, new_arm, cell['threads'])
        meta = read(cell['metadata']['path'])
        require(routing_cases.inspect_case(target)['body_sha256'] == meta['body_sha256'],
                'follow-up request preparation changed frozen data/model body')
        new_input = ref(target)
        frozen.append(new_input)
        cells_out.append({**cell, 'inputs': {**cell['inputs'], new_arm: new_input}})
    frozen.append(parent_binding)
    identities = dict(dependencies)
    for item in frozen:
        require(item['path'] not in identities or identities[item['path']] == item['sha256'],
                'new artifact/tool would replace a parent dependency')
        identities[item['path']] = item['sha256']
    plan = {'kind': kind, 'limits': limits, 'arms': arms,
            'families': FAMILIES, 'library_limits': LIBRARY_LIMITS,
            'parent_plan': parent_binding, 'parent_plan_sha256': routing.identity(parent),
            'parent_dependencies': dependencies, 'generator': parent['generator'],
            'harness': ref(harness), 'plugin': ref(plugin), 'cells': cells_out,
            'blocks': schedule([c['id'] for c in cells_out], arms=arms, limits=limits),
            'frozen_files': identities, 'root': str(root), 'created_utc': datetime.now(timezone.utc).isoformat()}
    validate(plan)
    save(root / 'plan.json', routing.seal(plan))
    return plan


def validate(plan):
    validate_contract(plan)
    if plan['kind'] in FOLLOWUP_KINDS:
        parent = routing.unseal(read(checked(plan['parent_plan'])))
        require(routing.identity(parent) == plan['parent_plan_sha256']
                and parent_dependencies(parent) == plan['parent_dependencies'], 'parent binding changed')
        require(plan['generator'] == parent['generator'], 'parent generator binding changed')
        for cell, original in zip(plan['cells'], parent['cells']):
            require({**cell, 'inputs': {a: cell['inputs'][a] for a in ARMS}} == original,
                    'follow-up study changed parent cases or original arm requests')
        for item in [plan['parent_plan'], *({'path': p, 'sha256': s}
                    for p, s in plan['parent_dependencies'].items())]:
            require(plan['frozen_files'].get(item['path']) == item['sha256'], 'unbound parent dependency')
    for path, sha in plan['frozen_files'].items():
        checked({'path': path, 'sha256': sha})
    for path in tool_files():
        require(plan['frozen_files'].get(str(path.resolve())) == digest(path), 'unbound current tool')
    for cell in plan['cells']:
        for item in [cell['metadata'], cell['witness'], *cell['inputs'].values()]:
            require(plan['frozen_files'].get(item['path']) == item['sha256'], 'unbound case dependency')
    for key in ['plugin', 'harness', 'generator']:
        require(plan['frozen_files'].get(plan[key]['path']) == plan[key]['sha256'], 'unbound executable')


def native(plan, cell, arm, directory, label, *, traced=False, phase_timing=False, cpus=None, timeout=120):
    target = directory / (label + '.json')
    log = target.with_suffix('.log')
    command = routing_run.native_command(Path(plan['harness']['path']), Path(plan['plugin']['path']),
                                         Path(cell['inputs'][arm]['path']), target, cell, traced)
    require(not phase_timing or traced, 'phase diagnostics require solver tracing')
    if phase_timing:
        command.insert(-5, 'FEREG_PHASE_TIMING=1')
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


def deferred_parent_controls(plan, parent, selected, output, started, controls):
    """Compare all four original arms directly, including the two unscored arms.

    Each of 96 comparisons has one fresh parent and one fresh candidate call.
    These records are independent of the three-arm scientific prerequisites.
    """
    limits = DEFERRED_LIMITS
    for cell in selected:
        meta = read(cell['metadata']['path'])
        for arm in ARMS:
            pair = {'cell': cell['id'], 'arm': arm, 'status': 'executed'}
            controls.append(pair)
            active = None
            try:
                for role, plugin in [('parent', parent['plugin']), ('candidate', plan['plugin'])]:
                    left = limits['science_budget_s'] - (time.monotonic() - started)
                    require(left > 0, 'science budget exhausted')
                    active = native({**plan, 'plugin': plugin}, cell, arm, output,
                                    f"{role}-control-{cell['id']}-{arm}",
                                    timeout=min(limits['call_timeout_s'], left))
                    active.update(phase=role + '_control', plugin=plugin)
                    pair[role] = active
                    _, result, _ = parse_run(active, meta)
                    active.update(result)
                require(pair['parent']['science_sha256'] == pair['candidate']['science_sha256'],
                        'parent/candidate control science or solver signature differs: '
                        + cell['id'] + '/' + arm)
                pair['status'] = pair['parent']['status'] = pair['candidate']['status'] = 'passed'
            except Exception as error:
                pair.update(status='failed', error=str(error))
                if active is not None and active['status'] == 'executed':
                    active.update(status='failed', error=str(error))
                raise


def science(plan, output, *, smoke=False):
    validate(plan)
    arms, limits = contract(plan)
    slots = int(os.environ.get('NSLOTS', '0'))
    require(os.environ.get('JOB_ID') and slots >= (2 if smoke else 4), 'science requires a compute allocation')
    output.mkdir(parents=True, exist_ok=False)
    selected = [plan['cells'][0]] if smoke else plan['cells']
    start = time.monotonic()
    fingerprints, records, baseline, worker_one = {}, [], {}, {}
    controls, active_row = [], None
    parent = routing.unseal(read(plan['parent_plan']['path'])) if plan.get('kind') in FOLLOWUP_KINDS else None
    status, failure = 'passed', None
    try:
        if plan.get('kind') == DEFERRED_KIND:
            deferred_parent_controls(plan, parent, selected, output, start, controls)
        for cell in selected:
            meta = read(cell['metadata']['path'])
            witness, _ = routing_oracle.load_reference(Path(cell['witness']['path']),
                                                        meta['case_sha256'], meta['body_sha256'])
            for arm in arms:
                previous = None
                control_fingerprint = None
                if plan.get('kind') == FACTOR_KIND and arm in ARMS:
                    left = limits['science_budget_s'] - (time.monotonic() - start)
                    require(left > 0, 'science budget exhausted')
                    active_row = native({**plan, 'plugin': parent['plugin']}, cell, arm, output,
                                        f"parent-control-{cell['id']}-{arm}",
                                        timeout=min(limits['call_timeout_s'], left))
                    active_row.update(phase='parent_control', plugin=parent['plugin'])
                    controls.append(active_row)
                    _, control, _ = parse_run(active_row, meta)
                    control_fingerprint = control['science_sha256']
                    active_row.update(control)
                for repeat in range(limits['science_repeats']):
                    left = limits['science_budget_s'] - (time.monotonic() - start)
                    require(left > 0, 'science budget exhausted')
                    row = native(plan, cell, arm, output, f"{cell['id']}-{arm}-{repeat}",
                                 traced=repeat == 0, timeout=min(limits['call_timeout_s'], left))
                    records.append(row)
                    active_row = row
                    raw, result, binary = parse_run(row, meta)
                    routing_oracle.check_reference(witness, raw, binary, meta)
                    key = cell['id'] + '/' + arm
                    if previous is not None:
                        require(previous == result['science_sha256'], 'fixed-worker repeat differs: ' + key)
                    previous = result['science_sha256']
                    if repeat == 0:
                        if plan.get('kind') == DEFERRED_KIND and arm in ARMS:
                            candidate_control = next(pair['candidate'] for pair in controls
                                                     if pair['cell'] == cell['id'] and pair['arm'] == arm)
                            require(candidate_control['science_sha256'] == result['science_sha256'],
                                    'candidate control/science fixed-worker repeat differs: ' + key)
                        if control_fingerprint is not None:
                            require(control_fingerprint == result['science_sha256'],
                                    'parent/candidate control science or solver signature differs: ' + key)
                            controls[-1]['status'] = 'passed'
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
        require(time.monotonic() - start <= limits['science_budget_s'], 'science budget exhausted')
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
        status, failure = 'failed', str(error)
        if active_row is not None and active_row['status'] == 'executed' and 'science_sha256' not in active_row:
            active_row.update(status='failed', error=failure)
    bound = [ref(p) for p in output.iterdir() if p.is_file()]
    receipt = {'kind': plan.get('kind', KIND) + '_science', 'status': status, 'failure': failure, 'smoke': smoke,
               'plan_sha256': routing.identity(plan), 'host': socket.gethostname(),
               'job_id': os.environ['JOB_ID'], 'slots': slots, 'fingerprints': fingerprints,
               'records': records, 'files': bound, 'seconds': time.monotonic() - start}
    if parent is not None:
        receipt.update(parent_controls=controls, parent_control_count=len(controls),
                       parent_control_status='passed' if status == 'passed' else 'failed')
        if plan.get('kind') == DEFERRED_KIND:
            receipt['parent_control_native_calls'] = sum(role in pair for pair in controls
                                                        for role in ['parent', 'candidate'])
    save(output / 'science.json', routing.seal(receipt))
    return receipt


def validate_science(plan, receipt):
    arms, limits = contract(plan)
    require(receipt['kind'] == plan.get('kind', KIND) + '_science' and receipt['status'] == 'passed'
            and receipt['smoke'] is False and receipt['plan_sha256'] == routing.identity(plan),
            'complete frozen science prerequisite required')
    require(set(receipt['fingerprints']) == {c['id'] + '/' + a for c in plan['cells'] for a in arms},
            'missing scientific prerequisite')
    for cell in plan['cells']:
        for arm in arms:
            rows = [r for r in receipt['records'] if r['cell'] == cell['id'] and r['arm'] == arm]
            require(len(rows) == limits['science_repeats'] and all(r['status'] == 'executed'
                    and r.get('science_sha256') == receipt['fingerprints'][cell['id'] + '/' + arm]
                    for r in rows) and 'resources' in rows[0], 'incomplete repeat/resource prerequisite')
    for item in receipt['files']:
        checked(item)
    if plan.get('kind') in FOLLOWUP_KINDS:
        controls = receipt['parent_controls']
        require(receipt['parent_control_status'] == 'passed'
                and receipt['parent_control_count'] == len(controls) == len(plan['cells']) * len(ARMS),
                'incomplete parent artifact controls')
        parent = routing.unseal(read(plan['parent_plan']['path']))
        if plan.get('kind') == DEFERRED_KIND:
            require(receipt['parent_control_native_calls'] == 2 * len(controls),
                    'incomplete parent/candidate control calls')
        for cell in plan['cells']:
            for arm in ARMS:
                rows = [r for r in controls if r['cell'] == cell['id'] and r['arm'] == arm]
                require(len(rows) == 1 and rows[0]['status'] == 'passed', 'incomplete parent artifact controls')
                if plan.get('kind') == DEFERRED_KIND:
                    pair = rows[0]
                    for role, plugin in [('parent', parent['plugin']), ('candidate', plan['plugin'])]:
                        row = pair[role]
                        require(row['status'] == 'passed' and row['cell'] == cell['id'] and row['arm'] == arm
                                and row['phase'] == role + '_control' and row['plugin'] == plugin,
                                'invalid parent/candidate control identity')
                    require(pair['parent']['science_sha256'] == pair['candidate']['science_sha256'],
                            'parent artifact control changed science or solver signature')
                    if arm in arms:
                        require(pair['candidate']['science_sha256'] == receipt['fingerprints'][cell['id'] + '/' + arm],
                                'candidate control/science fixed-worker repeat differs')
                else:
                    require(rows[0]['plugin'] == parent['plugin'] and rows[0]['phase'] == 'parent_control'
                            and rows[0]['science_sha256'] == receipt['fingerprints'][cell['id'] + '/' + arm],
                            'parent artifact control changed science or solver signature')


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


def summarize(rows, *, arms=None, reference='baseline', candidates=None):
    """Only complete paired blocks; all failures remain explicit, never excluded."""
    selected = ARMS if arms is None else arms
    candidates = [a for a in selected if a != reference] if candidates is None else candidates
    groups = {}
    for row in rows:
        if row.get('phase') == 'measure':
            groups.setdefault((row['cell'], row['block']), {})[row['arm']] = row
    pairs = {}
    for (cid, block), arms in groups.items():
        if set(arms) != set(selected) or any(r.get('status') != 'passed' for r in arms.values()):
            continue
        baseline = arms[reference]['seconds']
        for arm in candidates:
            candidate = arms[arm]['seconds']
            pairs.setdefault((cid, arm), []).append({'block': block, 'baseline_seconds': baseline,
                'candidate_seconds': candidate, 'ratio': candidate / baseline,
                'allowance_excess_seconds': candidate - baseline - max(.05 * baseline, .001)})
    return [{'cell': cid, 'arm': arm, **({'reference': reference} if reference != 'baseline' else {}),
             'paired_blocks': len(values), 'pairs': values,
             'median_ratio': statistics.median(v['ratio'] for v in values),
             'maximum_ratio': max(v['ratio'] for v in values),
             'median_allowance_excess_seconds': statistics.median(v['allowance_excess_seconds'] for v in values),
             'maximum_allowance_excess_seconds': max(v['allowance_excess_seconds'] for v in values)}
            for (cid, arm), values in sorted(pairs.items())]


def phase_diagnostics(log, result):
    """Retain phase records and account for cumulative Schur correction timers.

    A correction trace joins the next certificate record to its invocation.
    Otherwise a Schur record starts a new invocation, including after a failed
    attempt/fallback. No inference about untraced work follows from these sums.
    """
    phases, solver_trace, invocations = [], [], []
    correction = None
    for line in log.splitlines():
        if not line.startswith(('FEREG_PHASE|', 'FEREG_SOLVER_TRACE|')):
            continue
        parts = line.split('|')
        fields = {}
        for part in parts[2:] if parts[0] == 'FEREG_PHASE' else parts[1:]:
            key, separator, value = part.partition('=')
            require(separator and key and value and key not in fields, 'malformed phase/solver trace')
            fields[key] = value
        if parts[0] == 'FEREG_SOLVER_TRACE':
            solver_trace.append(fields)
            if fields.get('phase') == 'two_fe_pcg_correction':
                require(correction is None and invocations and not invocations[-1]['passed'],
                        'correction trace lacks its failed certificate')
                correction = int(fields['iterations'])
            continue
        phases.append({'phase': parts[1], 'fields': fields})
        if parts[1] != 'schur':
            continue
        times = {key: float(fields[key]) for key in
                 ['init', 'operator', 'recurrence', 'reconstruct', 'certificate']}
        require(all(math.isfinite(v) and v >= 0 for v in times.values()), 'invalid Schur phase time')
        iterations = int(fields['iterations'])
        require(iterations >= 0 and fields['passed'] in {'true', 'false'}, 'invalid Schur phase state')
        if correction is None:
            invocations.append({'cycles': 0, 'iterations': 0, 'passed': False,
                                **dict.fromkeys(times, 0.)})
        invocation = invocations[-1]
        if correction is not None:
            require(correction == invocation['iterations'] and iterations >= correction
                    and all(times[k] >= invocation[k] for k in ['operator', 'recurrence']),
                    'nonmonotonic cumulative Schur correction timers/iterations')
        for key in ['init', 'reconstruct', 'certificate']:
            invocation[key] += times[key]
        # The latest record is the cumulative total for this invocation, not
        # another increment. Across invocations counters are free to reset.
        invocation.update({key: times[key] for key in ['operator', 'recurrence']})
        invocation.update(cycles=invocation['cycles'] + 1, iterations=iterations,
                          passed=fields['passed'] == 'true')
        correction = None
    rows = result['solver_rows']
    require(len(rows) == 2 and all(len(row) == 16 for row in rows), 'invalid diagnostic solver rows')
    zero = all(row[2] == 0 for row in rows)
    partial = correction is not None or any(not row['passed'] for row in invocations) or any(row[10] for row in rows)
    schur_status = ('partial' if partial else 'recorded') if invocations else (
                   'absent_zero_iterations' if zero else 'absent_nonzero_iterations')
    return {'status': 'recorded' if phases else 'missing', 'phase_records': phases,
            'solver_trace': solver_trace, 'schur_status': schur_status,
            'schur_invocations': invocations,
            'schur_observed_seconds': {key: sum(row[key] for row in invocations)
                                      for key in ['init', 'operator', 'recurrence', 'reconstruct', 'certificate']}
                                      if invocations else None,
            'native_seconds': result['seconds'], 'rsolver_setup_seconds': [row[8] for row in rows],
            'rsolver_solve_seconds': [row[9] for row in rows],
            'coverage': 'observed certificate records only; failed attempts, route restarts, and fallback work may be untraced'}


def check_prerequisite(row, cell, receipt, *, traced=True):
    meta = read(cell['metadata']['path'])
    raw, result, binary = parse_run(row, meta)
    original = next(r for r in receipt['records'] if r['cell'] == cell['id'] and r['arm'] == row['arm'])
    reference, _, reference_columns = parse_run(original, meta)
    cmgschur.scientific_comparison(reference, raw, reference_columns, binary, meta)
    resource_log = Path(row['log'] if traced else original['log'])
    resources = routing_run.resource_diagnostics(resource_log.read_text(), row['arm'], False, {})
    if not traced:
        require(resources == original['resources'], 'traced prerequisite resource evidence changed')
        resources = {**resources, 'resource_basis': 'bound traced science prerequisite; current call untraced',
                     'prerequisite_log': ref(resource_log),
                     'prerequisite_science_sha256': original['science_sha256']}
    row['resources'] = resources
    return result


def collect_diagnostics(plan, receipt, output, cpus, observer, fingerprints, records):
    arms, limits = contract(plan)
    started = time.monotonic()
    selected = [cell for family in limits['diagnostic_families'] for cell in plan['cells']
                if cell['family'] == family]
    requests = ((cell, arm, repeat) for cell in selected for arm in arms
                for repeat in range(limits['diagnostic_repeats']))
    for cell, arm, repeat in requests:
        require(not observer.errors, 'isolation observer failed')
        remaining = limits['diagnostic_budget_s'] - (time.monotonic() - started)
        require(remaining > 0, 'diagnostic budget exhausted')
        row = native(plan, cell, arm, output, f"diagnostic-{cell['id']}-{arm}-{repeat}", cpus=cpus,
                     traced=True, phase_timing=True, timeout=min(limits['call_timeout_s'], remaining))
        row.update(phase='diagnostic', repeat=repeat)
        records.append(row)
        result = check_prerequisite(row, cell, receipt, traced=True)
        row.update(result)
        require(result['science_sha256'] == fingerprints[cell['id'] + '/' + arm],
                'diagnostic result changed same-allocation fixed-worker science')
        row['phase_diagnostics'] = phase_diagnostics(Path(row['log']).read_text(), result)
        require(row['phase_diagnostics']['status'] == 'recorded', 'missing diagnostic phase trace')
        row['status'] = 'passed'
    require(time.monotonic() - started <= limits['diagnostic_budget_s'], 'diagnostic budget exhausted')


def measure(plan, receipt, output):
    validate(plan)
    validate_science(plan, receipt)
    arms, limits = contract(plan)
    require({k: os.environ.get(k) for k in LIBRARY_LIMITS} == LIBRARY_LIMITS,
            'set frozen library thread limits before starting measurement')
    host = scc.discovery.host()
    output.mkdir(parents=True, exist_ok=False)
    records, failure, observer, started, ended = [], None, None, None, None
    before, queue, observed_started, observed_ended = None, None, None, None
    fingerprints, resource_bindings = {}, {}
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
            for arm in arms:
                require(not observer.errors, 'isolation observer failed')
                remaining = limits['warmup_budget_s'] - (time.monotonic() - warmup_started)
                require(remaining > 0, 'warmup budget exhausted')
                row = native(plan, cell, arm, output, f"warmup-{cell['id']}-{arm}", cpus=cpus,
                             traced=plan.get('kind') not in FOLLOWUP_KINDS,
                             timeout=min(limits['call_timeout_s'], remaining))
                row.update(phase='warmup')
                records.append(row)
                result = check_prerequisite(row, cell, receipt, traced=plan.get('kind') not in FOLLOWUP_KINDS)
                fingerprints[cell['id'] + '/' + arm] = result['science_sha256']
                if plan.get('kind') == DEFERRED_KIND:
                    resource_bindings[cell['id'] + '/' + arm] = row['resources']
                row.update(result, status='passed')
        require(time.monotonic() - warmup_started <= limits['warmup_budget_s'], 'warmup budget exhausted')
        started = time.monotonic_ns()
        for index, block in enumerate(plan['blocks']):
            observer.census(f'block-{index}-before')
            for slot in block:
                require(not observer.errors, 'isolation observer failed')
                remaining = limits['measurement_budget_s'] - (time.monotonic_ns() - started) / 1e9
                require(remaining > 0, 'measurement budget exhausted')
                cell, arm = by_id[slot['cell']], slot['arm']
                row = native(plan, cell, arm, output, f"block-{index}-{cell['id']}-{arm}", cpus=cpus,
                             timeout=min(limits['call_timeout_s'], remaining))
                row.update(phase='measure', block=index)
                if plan.get('kind') == DEFERRED_KIND:
                    row['resources'] = resource_bindings[cell['id'] + '/' + arm]
                records.append(row)
                # Verification between complete calls stops the campaign at its
                # first scientific failure. This never runs an oracle solve.
                _, result, _ = parse_run(row, read(cell['metadata']['path']))
                require(result['science_sha256'] == fingerprints[cell['id'] + '/' + arm],
                        'measured result changed same-allocation fixed-worker science')
                row.update(result, status='passed')
            observer.census(f'block-{index}-after')
        ended = time.monotonic_ns()
        require((ended - started) / 1e9 <= limits['measurement_budget_s'], 'measurement budget exhausted')
        if plan.get('kind') in FOLLOWUP_KINDS:
            observer.census('diagnostics-before')
            collect_diagnostics(plan, receipt, output, cpus, observer, fingerprints, records)
            observer.census('diagnostics-after')
            observed_ended = time.monotonic_ns()
    except Exception as error:
        failure = str(error)
        if records and records[-1]['status'] == 'executed':
            records[-1].update(status='failed', error=failure)
    finally:
        ended = ended or time.monotonic_ns()
        observed_ended = observed_ended or (time.monotonic_ns() if plan.get('kind') in FOLLOWUP_KINDS else ended)
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
                    observer.validate(started if started is not None else observed_started, observed_ended)
            except Exception as error:
                failure = failure or str(error)
    try:
        validate(plan)
        validate_science(plan, receipt)
    except (ValueError, OSError) as error:
        failure = failure or str(error)
    value = {'kind': plan.get('kind', KIND) + '_measurement', 'plan_sha256': routing.identity(plan),
             'science_sha256': routing.identity(receipt), 'status': 'failed' if failure else 'complete',
             'failure': failure, 'host': host, 'collection_pid': os.getpid(),
             'native_affinity': sorted(host['affinity'])[:4], 'started_monotonic_ns': started,
             'completed_monotonic_ns': ended, 'records': records, 'summaries': summarize(records, arms=arms),
             'files': [ref(p) for p in output.iterdir() if p.is_file()],
             'claim': limits['inference'], 'accounting': 'pending external qacct validation',
             'process_evidence': 'raw observations retained; competing-compute review required; no exclusions'}
    if plan.get('kind') == FACTOR_KIND:
        value['factor_comparisons'] = [summary for reference in ['cells_v2', 'cells']
                                      for summary in summarize(records, arms=arms, reference=reference,
                                                               candidates=['cells_v2_df'])]
    if plan.get('kind') == DEFERRED_KIND:
        value['deferred_comparisons'] = summarize(records, arms=arms, reference='cells',
                                                  candidates=['cells_deferred'])
    save(output / 'measurement.json', routing.seal(value))
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    prep = subs.add_parser('prepare')
    for name in ['root', 'generator', 'harness', 'plugin']:
        prep.add_argument('--' + name, type=Path, required=True)
    factor = subs.add_parser('prepare-factor', help='reuse a frozen four-arm parent; prepare only the D_f request')
    deferred = subs.add_parser('prepare-deferred',
                               help='reuse the frozen original four-arm parent; prepare only the deferred request')
    for sub in [factor, deferred]:
        for name in ['root', 'parent-plan', 'harness', 'plugin']:
            sub.add_argument('--' + name, type=Path, required=True)
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
    elif args.command == 'prepare-factor':
        prepare_factor(args.root, args.parent_plan, args.harness, args.plugin)
    elif args.command == 'prepare-deferred':
        prepare_deferred(args.root, args.parent_plan, args.harness, args.plugin)
    else:
        plan = routing.unseal(read(args.plan))
        result = (measure(plan, routing.unseal(read(args.science)), args.output) if args.command == 'measure'
                  else science(plan, args.output, smoke=args.command == 'smoke'))
        require(result['status'] in {'passed', 'complete'}, result.get('failure') or 'campaign failed')
        if 'parent_control_status' in result:
            count = (f"comparisons={result['parent_control_count']} calls={result['parent_control_native_calls']}"
                     if plan['kind'] == DEFERRED_KIND else f"calls={result['parent_control_count']}")
            print(f"COMPACT_CELLS_PARENT_CONTROLS status={result['parent_control_status']} {count}",
                  flush=True)
    print(f'COMPACT_CELLS_SUCCESS stage={args.command}', flush=True)


if __name__ == '__main__':
    main()
