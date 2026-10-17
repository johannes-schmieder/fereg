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
MEMBERSHIP_KIND = 'compact_cells_membership_study_v1'
MEMBERSHIP_ARMS = ['baseline', 'cells', 'cells_m', 'cells_m_lanes']
MEMBERSHIP_LIMITS = {**FACTOR_LIMITS, 'seed': 2026101702, 'blocks': 12}
OCCURRENCE_KIND = 'compact_cells_occurrence_study_v1'
OCCURRENCE_ARMS = ['baseline', 'cells', 'cells_m', 'cells_s']
OCCURRENCE_LIMITS = {**MEMBERSHIP_LIMITS, 'seed': 2026101722}
ADMISSION_KIND = 'compact_cells_admission_study_v1'
ADMISSION_ARMS = ['baseline', 'cells', 'cells_s', 'cells_s2']
ADMISSION_CONTROLS = [*ARMS, 'cells_v2_df', 'cells_deferred', 'cells_m', 'cells_m_lanes', 'cells_s']
ADMISSION_LIMITS = {**MEMBERSHIP_LIMITS, 'seed': 2026101732}
ENGINE_KIND = 'compact_cells_engine_study_v1'
ENGINE_ARMS = ['baseline', 'cells_s', 'cells_e', 'fit_owned', 'cells_e_final']
ENGINE_CONTROLS = [*ADMISSION_CONTROLS, 'cells_s2']
ENGINE_LIMITS = {**MEMBERSHIP_LIMITS, 'seed': 2026101742,
                 'diagnostic_repeats': 3, 'diagnostic_families': FAMILIES}
FOLLOWUP_KINDS = {FACTOR_KIND, DEFERRED_KIND, MEMBERSHIP_KIND, OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND}
PAIRED_CONTROL_KINDS = {DEFERRED_KIND, MEMBERSHIP_KIND, OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND}
MIXED_BATCH_KINDS = {MEMBERSHIP_KIND, OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND}
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
            DEFERRED_KIND: (DEFERRED_ARMS, DEFERRED_LIMITS),
            MEMBERSHIP_KIND: (MEMBERSHIP_ARMS, MEMBERSHIP_LIMITS),
            OCCURRENCE_KIND: (OCCURRENCE_ARMS, OCCURRENCE_LIMITS),
            ADMISSION_KIND: (ADMISSION_ARMS, ADMISSION_LIMITS),
            ENGINE_KIND: (ENGINE_ARMS, ENGINE_LIMITS)}[kind]


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
                  'routing_discovery.py', 'block_host.py', 'tests/test_compact_cells.py',
                  'occurrence_diagnostics.py', 'scheduler_probe.rs', 'tests/test_occurrence_diagnostics.py',
                  'prepare_scheduler_probe.py', 'tests/test_occurrence_cells.py',
                  'admission_confirmation.py', 'tests/test_admission_cells.py',
                  'engine_confirmation.py', 'tests/test_engine_cells.py',
                  'tests/test_engine_confirmation.py'])
    return sorted(files)


def schedule(cell_ids, *, arms=None, limits=None):
    """Fixed complete blocks; each cell has a seeded Latin rotation over arms.

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
        inputs = (set(ENGINE_CONTROLS) | set(arms) if plan['kind'] == ENGINE_KIND else
                  set(ADMISSION_CONTROLS) | set(arms) if plan['kind'] == ADMISSION_KIND else
                  set(arms) | set(ARMS) if plan['kind'] in FOLLOWUP_KINDS else set(arms))
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


def prepare_membership(root, parent_path, harness, plugin):
    return prepare_followup(root, parent_path, harness, plugin, MEMBERSHIP_KIND,
                            ['cells_m', 'cells_m_lanes'])


def prepare_followup(root, parent_path, harness, plugin, kind, new_arm, *, extra=None):
    arms, limits = contract({'kind': kind})
    new_arms = [new_arm] if isinstance(new_arm, str) else new_arm
    require(set(new_arms) == (set(arms) | (set(ENGINE_CONTROLS) if kind == ENGINE_KIND else set(ADMISSION_CONTROLS) if kind == ADMISSION_KIND else set())) - set(ARMS) and len(new_arms) == len(set(new_arms)),
            'follow-up must prepare exactly its new arms')
    parent_binding = ref(parent_path)
    parent = routing.unseal(read(parent_path))
    dependencies = parent_dependencies(parent)
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=False)
    frozen = [ref(path) for path in [harness, plugin, *tool_files()]]
    cells_out = []
    for cell in parent['cells']:
        inputs = dict(cell['inputs'])
        meta = read(cell['metadata']['path'])
        for arm in new_arms:
            target = root / f"{cell['id']}-{arm}.case"
            routing.prepare(checked(cell['inputs']['baseline']), target, arm, cell['threads'])
            require(routing_cases.inspect_case(target)['body_sha256'] == meta['body_sha256'],
                    'follow-up request preparation changed frozen data/model body')
            inputs[arm] = ref(target)
            frozen.append(inputs[arm])
        cells_out.append({**cell, 'inputs': inputs})
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
            'frozen_files': identities, 'root': str(root), 'created_utc': datetime.now(timezone.utc).isoformat(),
            'prepared_wall_ns': time.time_ns(), **(extra or {})}
    if extra:
        require(kind in {OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND}, 'unsupported extra bindings')
        if kind == OCCURRENCE_KIND:
            item = extra['scheduler_diagnostic_admission']
            plan['frozen_files'][item['path']] = item['sha256']
            plan['frozen_files'].update(occurrence_control_bindings(plan))
        elif kind == ADMISSION_KIND:
            plan['frozen_files'].update(admission_control_bindings(plan))
        else:
            plan['frozen_files'].update(engine_control_bindings(plan))
    validate(plan)
    save(root / 'plan.json', routing.seal(plan))
    return plan


def occurrence_affinity(admission):
    import occurrence_diagnostics as diagnostic
    return diagnostic.validate_admission(admission)['affinity_cores']


def occurrence_parent(plan):
    admission = routing.unseal(read(checked(plan['scheduler_diagnostic_admission'])))
    receipt = routing.unseal(read(checked(admission['diagnostic'])))
    diagnostic = routing.unseal(read(checked(receipt['plan'])))
    return routing.unseal(read(checked(diagnostic['parent_plan'])))


def occurrence_control_bindings(plan):
    reference=occurrence_parent(plan)
    original=routing.unseal(read(checked(plan['parent_plan'])))
    require(reference['kind']==MEMBERSHIP_KIND, 'wrong prior membership control artifact')
    validate_contract(reference)
    items=[reference[key] for key in ['plugin','harness']]
    require(reference['harness']==plan['harness'], 'prior control harness differs')
    for cell,old in zip(reference['cells'],original['cells']):
        require({**cell,'inputs':{a:cell['inputs'][a] for a in ARMS}}==old,
                'prior membership control data/model differs')
        meta=read(checked(cell['metadata']))
        items.append(cell['metadata'])
        for arm in ['cells_m','cells_m_lanes']:
            item=cell['inputs'][arm];items.append(item)
            require(routing_cases.inspect_case(checked(item))['body_sha256']==meta['body_sha256'],
                    'prior membership request body changed')
    for item in items:
        require(reference['frozen_files'].get(item['path'])==item['sha256'], 'unbound prior membership control')
        checked(item)
    return {item['path']:item['sha256'] for item in items}


def science_affinity(plan):
    if plan.get('kind') not in {OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND}:
        return None
    cores = affinity_cores(plan)
    require(int(os.environ.get('NSLOTS', '0')) == cores, 'science requires frozen affinity slot count')
    cpus = sorted(os.sched_getaffinity(0))[:cores]
    require(len(cpus) == cores, 'insufficient science affinity')
    return cpus


def control_arms(plan):
    if plan.get('kind') == ENGINE_KIND:
        return ENGINE_CONTROLS
    if plan.get('kind') == ADMISSION_KIND:
        return ADMISSION_CONTROLS
    return [*ARMS, 'cells_m', 'cells_m_lanes'] if plan.get('kind') == OCCURRENCE_KIND else ARMS


def control_parent(plan, original, arm):
    if plan.get("kind") == ENGINE_KIND:
        return routing.unseal(read(checked(plan["prior_admission_plan"])))
    if plan.get("kind") == ADMISSION_KIND:
        return routing.unseal(read(checked(plan["prior_occurrence_plan"])))
    return occurrence_parent(plan) if arm in ['cells_m', 'cells_m_lanes'] else original


def admission_control_bindings(plan):
    item = plan['prior_occurrence_plan']
    prior = routing.unseal(read(checked(item)))
    validate_contract(prior)
    require(prior['kind'] == OCCURRENCE_KIND and prior['harness'] == plan['harness'],
            'S2 requires the frozen S artifact and unchanged harness')
    original = routing.unseal(read(checked(plan['parent_plan'])))
    require(len(prior['cells']) == len(original['cells']), 'S parent coverage differs')
    for cell, old in zip(prior['cells'], original['cells']):
        require({**cell, 'inputs': {a:cell['inputs'][a] for a in ARMS}} == old,
                'S parent data/model differs')
    items = [item, prior['plugin'], prior['harness']]
    for value in items: checked(value)
    return {value['path']: value['sha256'] for value in items}


def validate_engine_request(path, arm, threads):
    from native_io import case_header
    with Path(path).open('rb') as stream:
        _, args = case_header(stream)
    pairs = [argument.decode().split('=', 1) for argument in args[1:]]
    require(all(len(pair) == 2 for pair in pairs), 'malformed engine request')
    fields = dict(pairs)
    require(args[0] == b'estimate-ols' and len(fields) == len(pairs)
            and fields.get('solver') == fields.get('preconditioner') == 'auto'
            and fields.get('threads') == str(threads)
            and fields.get('inject') == (None if arm == 'baseline' else routing.ABLATIONS[arm])
            and 'break_marker_token' not in fields, 'engine request injection/solver/worker binding changed')


def engine_control_bindings(plan):
    item = plan['prior_admission_plan']
    prior = routing.unseal(read(checked(item)))
    validate_contract(prior)
    require(prior['kind'] == ADMISSION_KIND and prior['harness'] == plan['harness'],
            'engine requires the frozen S2 artifact and unchanged harness')
    original = routing.unseal(read(checked(plan['parent_plan'])))
    require(len(prior['cells']) == len(original['cells']), 'S2 parent coverage differs')
    items = [item, prior['plugin'], prior['harness']]
    for cell, old in zip(prior['cells'], original['cells']):
        require({**cell, 'inputs': {a: cell['inputs'][a] for a in ARMS}} == old,
                'S2 parent data/model differs')
        for arm in ENGINE_CONTROLS:
            request = cell['inputs'][arm]
            require(prior['frozen_files'].get(request['path']) == request['sha256'],
                    'unbound prior S2 control request')
            validate_engine_request(checked(request), arm, cell['threads'])
            items.append(request)
    for value in items:
        checked(value)
    require(all(prior['frozen_files'].get(value['path']) == value['sha256']
                for value in items[1:]), 'unbound prior S2 artifact')
    return {value['path']: value['sha256'] for value in items}


def prepare_engine(root, parent_path, harness, plugin, prior_path):
    return prepare_followup(root, parent_path, harness, plugin, ENGINE_KIND,
        [arm for arm in dict.fromkeys([*ENGINE_CONTROLS, *ENGINE_ARMS]) if arm not in ARMS],
        extra={'prior_admission_plan': ref(prior_path), 'native_affinity_cores': 4})


def prepare_admission(root, parent_path, harness, plugin, prior_path):
    return prepare_followup(root, parent_path, harness, plugin, ADMISSION_KIND,
        [a for a in [*ADMISSION_CONTROLS, 'cells_s2'] if a not in ARMS],
        extra={'prior_occurrence_plan':ref(prior_path), 'native_affinity_cores':4})


def prepare_occurrence(root, parent_path, harness, plugin, admission_path):
    admission = ref(admission_path)
    cores = occurrence_affinity(routing.unseal(read(admission_path)))
    return prepare_followup(root, parent_path, harness, plugin, OCCURRENCE_KIND,
                            ['cells_m', 'cells_s'], extra={'scheduler_diagnostic_admission': admission,
                                                        'native_affinity_cores': cores})


def affinity_cores(plan):
    return plan['native_affinity_cores'] if plan.get('kind') in {OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND} else 4


def validate(plan):
    validate_contract(plan)
    if plan['kind'] == ENGINE_KIND:
        require(plan['native_affinity_cores'] == 4 and all(plan['frozen_files'].get(path) == sha
                for path, sha in engine_control_bindings(plan).items()), 'unbound engine prior artifact or affinity')
    if plan['kind'] == ADMISSION_KIND:
        require(plan['native_affinity_cores'] == 4 and all(plan['frozen_files'].get(path) == sha
                for path,sha in admission_control_bindings(plan).items()), 'unbound S2 prior artifact or affinity')
    if plan['kind'] == OCCURRENCE_KIND:
        require(all(plan['frozen_files'].get(path)==sha for path,sha in occurrence_control_bindings(plan).items()),
                'unbound prior membership control dependency')
        admission = plan['scheduler_diagnostic_admission']
        require(plan['frozen_files'].get(admission['path']) == admission['sha256']
                and plan['native_affinity_cores'] == occurrence_affinity(routing.unseal(read(checked(admission)))),
                'occurrence diagnostic/affinity binding changed')
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
        if plan['kind'] == ENGINE_KIND:
            for arm, item in cell['inputs'].items():
                validate_engine_request(checked(item), arm, cell['threads'])
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
    These records are independent of the scored-arm scientific prerequisites.
    """
    _, limits = contract(plan)
    cpus = science_affinity(plan)
    for cell in selected:
        meta = read(cell['metadata']['path'])
        for arm in control_arms(plan):
            reference = control_parent(plan, parent, arm)
            request = next(c for c in reference['cells'] if c['id'] == cell['id']) if arm in ['cells_m', 'cells_m_lanes'] and plan.get('kind') not in {ADMISSION_KIND, ENGINE_KIND} else cell
            pair = {'cell': cell['id'], 'arm': arm, 'status': 'executed'}
            controls.append(pair)
            active = None
            try:
                for role, plugin in [('parent', reference['plugin']), ('candidate', plan['plugin'])]:
                    left = limits['science_budget_s'] - (time.monotonic() - started)
                    require(left > 0, 'science budget exhausted')
                    active = native({**plan, 'plugin': plugin}, request, arm, output,
                                    f"{role}-control-{cell['id']}-{arm}", cpus=cpus,
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


def engine_confirmation_bindings(plan, confirmation_path, driver_path):
    import engine_confirmation as confirmation
    import membership_confirmation_run as execution
    require(confirmation_path is not None and driver_path is not None,
            'engine candidate calls require the frozen C4 plan and driver')
    bindings = {'plan': ref(confirmation_path), 'driver': ref(driver_path)}
    frozen = routing.unseal(read(checked(bindings['plan'])))
    driver = routing.unseal(read(checked(bindings['driver'])))
    confirmation.validate(frozen)
    execution.validate_driver(frozen, driver)
    require(frozen['discovery_plan_sha256'] == routing.identity(plan), 'C4 belongs to another D4 plan')
    require(max(frozen['prepared_wall_ns'], driver['frozen_wall_ns']) < time.time_ns(),
            'C4 freeze chronology is invalid')
    return bindings


def engine_exactness(fingerprints, cells):
    for cell in cells:
        cid = cell['id']
        for left, right in [('baseline', 'fit_owned'), ('cells_e', 'cells_e_final')]:
            require(fingerprints[cid + '/' + left] == fingerprints[cid + '/' + right],
                    'engine B ownership changed exact science or solver signatures: ' + cid + '/' + right)


def science(plan, output, *, smoke=False, confirmation_plan=None, confirmation_driver=None):
    validate(plan)
    confirmation = (engine_confirmation_bindings(plan, confirmation_plan, confirmation_driver)
                    if plan.get('kind') == ENGINE_KIND else None)
    arms, limits = contract(plan)
    slots = int(os.environ.get('NSLOTS', '0'))
    require(os.environ.get('JOB_ID') and slots >= (2 if smoke else 4), 'science requires a compute allocation')
    output.mkdir(parents=True, exist_ok=False)
    cpus = science_affinity(plan)
    selected = [plan['cells'][0]] if smoke else plan['cells']
    start = time.monotonic()
    fingerprints, records, baseline, worker_one = {}, [], {}, {}
    controls, active_row = [], None
    parent = routing.unseal(read(plan['parent_plan']['path'])) if plan.get('kind') in FOLLOWUP_KINDS else None
    status, failure = 'passed', None
    try:
        if plan.get('kind') in PAIRED_CONTROL_KINDS:
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
                                 traced=repeat == 0, cpus=cpus, timeout=min(limits['call_timeout_s'], left))
                    records.append(row)
                    active_row = row
                    raw, result, binary = parse_run(row, meta)
                    routing_oracle.check_reference(witness, raw, binary, meta)
                    if plan.get('kind') in MIXED_BATCH_KINDS:
                        row.update(repeat=repeat, oracle_status='passed',
                                   model_discrete_sha256=routing_run.model_discrete_sha256(raw))
                    key = cell['id'] + '/' + arm
                    if previous is not None:
                        require(previous == result['science_sha256'], 'fixed-worker repeat differs: ' + key)
                    previous = result['science_sha256']
                    if repeat == 0:
                        if plan.get('kind') in PAIRED_CONTROL_KINDS and arm in ARMS:
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
                    if plan.get('kind') == ENGINE_KIND and arm in {'fit_owned', 'cells_e_final'}:
                        reference_arm = 'baseline' if arm == 'fit_owned' else 'cells_e'
                        require(fingerprints[cell['id'] + '/' + reference_arm] == result['science_sha256'],
                                'engine B ownership changed exact science or solver signatures: ' + key)
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
               'job_id': os.environ['JOB_ID'], 'slots': slots, **({'native_affinity_cores': affinity_cores(plan)} if plan.get('kind') in {OCCURRENCE_KIND, ADMISSION_KIND, ENGINE_KIND} else {}), 'fingerprints': fingerprints,
               'records': records, 'files': bound, 'seconds': time.monotonic() - start}
    if confirmation is not None:
        receipt['confirmation'] = confirmation
    if parent is not None:
        receipt.update(parent_controls=controls, parent_control_count=len(controls),
                       parent_control_status='passed' if status == 'passed' else 'failed')
        if plan.get('kind') in PAIRED_CONTROL_KINDS:
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
    if plan.get('kind') in MIXED_BATCH_KINDS:
        validate_membership_science(plan, receipt)
    if plan.get('kind') == ENGINE_KIND:
        require([(r['cell'], r['arm'], r.get('repeat')) for r in receipt['records']] ==
                [(cell['id'], arm, repeat) for cell in plan['cells'] for arm in arms
                 for repeat in range(limits['science_repeats'])], 'engine science order changed')
        require([(r['cell'], r['arm']) for r in receipt['parent_controls']] ==
                [(cell['id'], arm) for cell in plan['cells'] for arm in ENGINE_CONTROLS],
                'engine prior-control order changed')
        engine_exactness(receipt['fingerprints'], plan['cells'])
        binding = receipt.get('confirmation', {})
        require(engine_confirmation_bindings(plan, binding.get('plan', {}).get('path'),
                binding.get('driver', {}).get('path')) == binding, 'C4 prerequisite binding changed')
    if plan.get('kind') == ADMISSION_KIND:
        require(all(receipt['fingerprints'][cell['id']+'/cells_s2'] ==
                    receipt['fingerprints'][cell['id']+'/cells_s'] for cell in plan['cells']),
                'S2 changed S science or solver signatures on the frozen panel')
    for item in receipt['files']:
        checked(item)
    if plan.get('kind') in FOLLOWUP_KINDS:
        controls = receipt['parent_controls']
        require(receipt['parent_control_status'] == 'passed'
                and receipt['parent_control_count'] == len(controls) == len(plan['cells']) * len(control_arms(plan)),
                'incomplete parent artifact controls')
        parent = routing.unseal(read(plan['parent_plan']['path']))
        if plan.get('kind') in PAIRED_CONTROL_KINDS:
            require(receipt['parent_control_native_calls'] == 2 * len(controls),
                    'incomplete parent/candidate control calls')
        for cell in plan['cells']:
            for arm in control_arms(plan):
                reference = control_parent(plan, parent, arm)
                rows = [r for r in controls if r['cell'] == cell['id'] and r['arm'] == arm]
                require(len(rows) == 1 and rows[0]['status'] == 'passed', 'incomplete parent artifact controls')
                if plan.get('kind') in PAIRED_CONTROL_KINDS:
                    pair = rows[0]
                    for role, plugin in [('parent', reference['plugin']), ('candidate', plan['plugin'])]:
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


def validate_membership_science(plan, receipt):
    """Retain complete oracle/repeat evidence and exact model decisions.

    New-arm whole-fit hashes may differ from Auto/cells because each batch can
    select its own operator. The existing scientific comparator and independent
    oracle still apply, without widening either numerical envelope.
    """
    arms, limits = contract(plan)
    rows = receipt['records']
    expected = {(cell['id'], arm, repeat) for cell in plan['cells'] for arm in arms
                for repeat in range(limits['science_repeats'])}
    require(len(rows) == len(expected) and
            {(row['cell'], row['arm'], row.get('repeat')) for row in rows} == expected,
            'incomplete membership science records')
    decisions = {}
    for row in rows:
        require(row['label'] == f"{row['cell']}-{row['arm']}-{row['repeat']}"
                and row.get('oracle_status') == 'passed'
                and isinstance(row.get('model_discrete_sha256'), str)
                and re.fullmatch(r'[0-9a-f]{64}', row['model_discrete_sha256']) is not None,
                'missing membership oracle/discrete evidence')
        family = next(cell['family'] for cell in plan['cells'] if cell['id'] == row['cell'])
        previous = decisions.setdefault(family, row['model_discrete_sha256'])
        require(row['model_discrete_sha256'] == previous,
                'membership cross-arm/worker model-discrete decisions differ')


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
        cpus = sorted(host['affinity'])[:affinity_cores(plan)]
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
                if plan.get('kind') in PAIRED_CONTROL_KINDS:
                    resource_bindings[cell['id'] + '/' + arm] = row['resources']
                row.update(result, status='passed')
        require(time.monotonic() - warmup_started <= limits['warmup_budget_s'], 'warmup budget exhausted')
        if plan.get('kind') == ENGINE_KIND:
            engine_exactness(fingerprints, plan['cells'])
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
                if plan.get('kind') in PAIRED_CONTROL_KINDS:
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
             'native_affinity': sorted(host['affinity'])[:affinity_cores(plan)], 'started_monotonic_ns': started,
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
    if plan.get('kind') == OCCURRENCE_KIND:
        value['occurrence_comparisons'] = [v for reference in ['cells', 'cells_m']
            for v in summarize(records, arms=arms, reference=reference, candidates=['cells_s'])]
    if plan.get('kind') == MEMBERSHIP_KIND:
        value['membership_comparisons'] = summarize(records, arms=arms, reference='cells',
                                                    candidates=['cells_m', 'cells_m_lanes'])
    save(output / 'measurement.json', routing.seal(value))
    return value


def cell_gate(plan, science_receipt, measurement, *, kind, finalist, gains_required, ratio_limit, seconds_limit=None):
    """Evaluate the frozen descriptive D1 gate for the predetermined M arm.

    Every setting needs all twelve pairs. Gains exceed the symmetric frozen
    max(5%, 1 ms) allowance; regressions exceed the ordinary upper allowance.
    Lane attribution never supplies a fallback finalist.
    """
    require(plan.get('kind') == kind, 'membership gate requires its discovery plan')
    validate(plan)
    validate_science(plan, science_receipt)
    require(measurement.get('kind') == kind + '_measurement'
            and measurement.get('status') == 'complete'
            and measurement.get('plan_sha256') == routing.identity(plan)
            and measurement.get('science_sha256') == routing.identity(science_receipt),
            'complete bound membership measurement required')
    arms, limits = contract(plan)
    cells = {cell['id']: cell for cell in plan['cells']}
    expected = {(f"warmup-{cid}-{arm}", cid, arm, 'warmup') for cid in cells for arm in arms}
    expected.update((f"block-{block}-{cid}-{arm}", cid, arm, 'measure')
                    for block in range(limits['blocks']) for cid in cells for arm in arms)
    expected.update((f"diagnostic-{cid}-{arm}-{repeat}", cid, arm, 'diagnostic')
                    for cid, cell in cells.items() if cell['family'] in limits['diagnostic_families']
                    for arm in arms for repeat in range(limits['diagnostic_repeats']))
    rows = measurement['records']
    require(len(rows) == len(expected) and
            {(row['label'], row['cell'], row['arm'], row['phase']) for row in rows} == expected,
            'incomplete membership measurement records')
    warmups = {(row['cell'], row['arm']): row for row in rows if row['phase'] == 'warmup'}
    for row in rows:
        require(row['status'] == 'passed' and math.isfinite(row['seconds']) and row['seconds'] > 0
                and row['science_sha256'] == warmups[(row['cell'], row['arm'])]['science_sha256'],
                'invalid membership measurement science/time')
        if row['phase'] == 'measure':
            require(type(row.get('block')) is int
                    and row['label'] == f"block-{row['block']}-{row['cell']}-{row['arm']}",
                    'invalid membership measurement block')
        if row['phase'] == 'diagnostic':
            require(type(row.get('repeat')) is int
                    and row['label'] == f"diagnostic-{row['cell']}-{row['arm']}-{row['repeat']}",
                    'invalid membership diagnostic repeat')
    for item in measurement['files']:
        checked(item)
    comparisons = summarize(rows, arms=arms, candidates=[finalist])
    require(len(comparisons) == len(cells)
            and all(row['paired_blocks'] == limits['blocks'] for row in comparisons),
            'incomplete membership paired settings')
    for row in comparisons:
        row['median_gain_excess_seconds'] = statistics.median(
            pair['candidate_seconds'] - pair['baseline_seconds']
            + max(.05 * pair['baseline_seconds'], .001) for pair in row['pairs'])
        row['material_gain'] = row['median_gain_excess_seconds'] < 0
        row['regression'] = row['median_allowance_excess_seconds'] > 0
    builder = []
    for cid, cell in cells.items():
        if cell['family'] not in limits['diagnostic_families']:
            continue
        totals = {}
        for arm in ['cells', finalist]:
            totals[arm] = []
            for row in rows:
                if (row['cell'], row['arm'], row['phase']) != (cid, arm, 'diagnostic'):
                    continue
                traces = [trace for trace in row['phase_diagnostics']['solver_trace']
                          if trace.get('phase') == 'auto_kernel_route']
                require(traces and all(trace.get('preset') == 'routing_' + arm for trace in traces),
                        'missing membership builder trace')
                times = [float(trace['builder_seconds']) for trace in traces]
                require(all(math.isfinite(value) and value >= 0 for value in times),
                        'invalid membership builder time')
                totals[arm].append(sum(times))
        require(all(len(values) == limits['diagnostic_repeats'] for values in totals.values()),
                'incomplete membership builder diagnostics')
        original, candidate = [statistics.median(totals[arm]) for arm in ['cells', finalist]]
        builder.append({'cell': cid, 'per_fit_seconds': totals,
                        'original_median_seconds': original, 'candidate_median_seconds': candidate,
                        'ratio': candidate / original if original > 0 else None,
                        'passed': original > 0 and ((candidate < ratio_limit * original) if seconds_limit is None
                            else (candidate <= ratio_limit * original and candidate <= seconds_limit))})
    gains = sum(row['material_gain'] for row in comparisons)
    regressions = sum(row['regression'] for row in comparisons)
    return {'kind': kind + '_gate', 'plan_sha256': routing.identity(plan),
            'science_sha256': routing.identity(science_receipt),
            'measurement_sha256': routing.identity(measurement), 'finalist': finalist,
            'status': 'eligible' if regressions == 0 and gains >= gains_required and all(row['passed'] for row in builder)
                      else 'ineligible',
            'material_gains': gains, 'regressions': regressions,
            'comparisons': comparisons, 'builder_comparisons': builder,
            'claim': 'descriptive discovery gate only; untouched confirmation remains required'}


def membership_gate(plan, science_receipt, measurement):
    return cell_gate(plan, science_receipt, measurement, kind=MEMBERSHIP_KIND,
                     finalist='cells_m', gains_required=12, ratio_limit=.5)


def occurrence_gate(plan, science_receipt, measurement):
    return cell_gate(plan, science_receipt, measurement, kind=OCCURRENCE_KIND,
                     finalist='cells_s', gains_required=16, ratio_limit=.35, seconds_limit=.0015)


def admission_gate(plan, science_receipt, measurement):
    gate = cell_gate(plan, science_receipt, measurement, kind=ADMISSION_KIND,
                    finalist='cells_s2', gains_required=16, ratio_limit=math.inf)
    rows = measurement['records']
    warmups = {(r['cell'], r['arm']): r['science_sha256'] for r in rows if r['phase'] == 'warmup'}
    require(all(warmups[(cell['id'], 'cells_s2')] == warmups[(cell['id'], 'cells_s')]
                for cell in plan['cells']), 'S2 changed S science on the measurement host')
    prior = summarize(rows, arms=ADMISSION_ARMS, reference='cells_s', candidates=['cells_s2'])
    require(len(prior) == len(plan['cells']) and all(x['paired_blocks'] == ADMISSION_LIMITS['blocks'] for x in prior),
            'incomplete S2 versus S comparisons')
    gate['comparisons_vs_s'] = prior
    builders = []
    for old in gate['builder_comparisons']:
        cid = old['cell']; totals = {}; outcomes = []
        for arm in ['cells', 'cells_s', 'cells_s2']:
            selected = [r for r in rows if (r['cell'],r['arm'],r['phase']) == (cid,arm,'diagnostic')]
            require(len(selected) == ADMISSION_LIMITS['diagnostic_repeats'], 'missing S2 diagnostics')
            totals[arm] = []
            for row in selected:
                traces = row['phase_diagnostics']['solver_trace']
                builds = [t for t in traces if t.get('phase') == 'auto_kernel_route']
                require(builds and all(t['preset'] == 'routing_'+arm for t in builds), 'wrong builder attribution')
                values = [float(t['builder_seconds']) for t in builds]
                require(all(math.isfinite(v) and v >= 0 for v in values), 'invalid S2 builder time')
                totals[arm].append(sum(values))
                if arm == 'cells_s2':
                    installed = len(builds)==1 and builds[0].get('installed') == '1'
                    refused = len(builds)==1 and builds[0].get('installed') == '0' and builds[0].get('refusal','').startswith('Some(Unprofitable')
                    require(installed or refused, 'S2 mechanism requires one build or structural refusal')
                    if refused:
                        proofs = [t for t in traces if t.get('phase') == 'auto_cells_refusal']
                        require(len(proofs)==1 and int(proofs[0]['cells'])>int(proofs[0]['limit']), 'missing structural refusal proof')
                        decisions=[t for t in traces if t.get('phase')=='auto_cells_s2']
                        require(decisions and all(t['used_operator']=='0' for t in decisions), 'refused cells were applied')
                    outcomes.append('refused' if refused else 'accepted')
        require(len(set(outcomes)) == 1, 'S2 refusal decision differs across repeats')
        original, reference, candidate = [statistics.median(totals[a]) for a in ['cells','cells_s','cells_s2']]
        refused = outcomes[0] == 'refused'
        passed = reference>0 and original>0 and (candidate <= .6*reference and candidate <= .0015 and candidate <= .35*original
                 if refused else candidate <= 1.1*reference)
        builders.append({'cell':cid,'outcome':outcomes[0],'per_fit_seconds':totals,
                         'original_median_seconds':original,'s_median_seconds':reference,'candidate_median_seconds':candidate,
                         'ratio_vs_s':candidate/reference if reference>0 else None,
                         'ratio_vs_original':candidate/original if original>0 else None,'passed':passed})
    gate['builder_comparisons'] = builders
    gate['regressions_vs_s'] = sum(x['median_allowance_excess_seconds'] > 0 for x in prior)
    gate['status'] = 'eligible' if gate['regressions']==gate['regressions_vs_s']==0 and gate['material_gains']>=16 and all(x['passed'] for x in builders) else 'ineligible'
    return gate


def engine_measurement_order(plan):
    arms, limits = contract(plan)
    rows = [(f"warmup-{cell['id']}-{arm}", cell['id'], arm, 'warmup', None, None)
            for cell in plan['cells'] for arm in arms]
    rows += [(f"block-{block}-{slot['cell']}-{slot['arm']}", slot['cell'], slot['arm'],
              'measure', block, None) for block, slots in enumerate(plan['blocks']) for slot in slots]
    rows += [(f"diagnostic-{cell['id']}-{arm}-{repeat}", cell['id'], arm, 'diagnostic', None, repeat)
             for family in limits['diagnostic_families'] for cell in plan['cells'] if cell['family'] == family
             for arm in arms for repeat in range(limits['diagnostic_repeats'])]
    return rows


def engine_projection_uses_cells(traces, arm):
    """Attribute batches by their terminal action, excluding final residual work."""
    phase = 'auto_cells_e' if arm == 'cells_e_final' else 'auto_cells_s'
    pending, decisions, actions = [], [], []
    for trace in traces:
        if trace.get('phase') == phase:
            require(trace.get('preset') == 'routing_' + arm and trace.get('used_operator') in {'0', '1'},
                    'invalid engine projection decision')
            pending.append(trace)
        if trace.get('phase') == 'routing_action':
            require(trace.get('preset') == 'routing_' + arm and trace.get('final') in {'0', '1'}
                    and trace.get('certified') == '1', 'invalid engine projection action')
            if trace['final'] == '0':
                require(pending or trace.get('iterations') == '0',
                        'missing engine decision for nonzero projection')
                decisions.extend(pending)
                actions.append(trace)
            pending = []
    require(actions, 'missing engine first-projection action')
    return any(decision['used_operator'] == '1' for decision in decisions)


def engine_gate(plan, science_receipt, measurement):
    """D4's one fixed A+B finalist; all observations and both B exactness pairs."""
    require(plan.get('kind') == ENGINE_KIND, 'engine gate requires D4')
    validate(plan)
    validate_science(plan, science_receipt)
    require(measurement.get('kind') == ENGINE_KIND + '_measurement'
            and measurement.get('status') == 'complete'
            and measurement.get('plan_sha256') == routing.identity(plan)
            and measurement.get('science_sha256') == routing.identity(science_receipt),
            'complete bound engine measurement required')
    rows = measurement['records']
    actual = [(r['label'], r['cell'], r['arm'], r['phase'], r.get('block'), r.get('repeat')) for r in rows]
    require(actual == engine_measurement_order(plan), 'engine measurement coverage/order changed')
    fingerprints = {r['cell'] + '/' + r['arm']: r['science_sha256'] for r in rows if r['phase'] == 'warmup'}
    engine_exactness(fingerprints, plan['cells'])
    for row in rows:
        require(row['status'] == 'passed' and math.isfinite(row['seconds']) and row['seconds'] > 0
                and row['science_sha256'] == fingerprints[row['cell'] + '/' + row['arm']],
                'invalid engine measurement science/time')
    for item in measurement['files']:
        checked(item)
    comparisons = {}
    for name, reference, candidate in [('finalist_vs_auto', 'baseline', 'cells_e_final'),
                                      ('finalist_vs_s', 'cells_s', 'cells_e_final'),
                                      ('owned_vs_auto', 'baseline', 'fit_owned')]:
        values = summarize(rows, arms=ENGINE_ARMS, reference=reference, candidates=[candidate])
        require(len(values) == len(plan['cells']) and all(v['paired_blocks'] == 12 for v in values),
                'incomplete engine paired settings')
        for value in values:
            value['material_gain'] = statistics.median(
                pair['candidate_seconds'] - pair['baseline_seconds'] + max(.05 * pair['baseline_seconds'], .001)
                for pair in value['pairs']) < 0
            value['regression'] = value['median_allowance_excess_seconds'] > 0
        comparisons[name] = values
    timers = []
    for cell in plan['cells']:
        cid = cell['id']
        selected = {arm: [r for r in rows if (r['cell'], r['arm'], r['phase']) == (cid, arm, 'diagnostic')]
                    for arm in ['cells_s', 'cells_e_final']}
        scored = {arm: [r for r in rows if (r['cell'], r['arm'], r['phase']) == (cid, arm, 'measure')]
                  for arm in selected}
        times, classifications = {}, {}
        for arm, measurements in scored.items():
            require(len(measurements) == ENGINE_LIMITS['blocks'], 'incomplete scored first-solver timers')
            times[arm] = []
            for row in measurements:
                solver = row['solver_rows'][0]
                require(len(solver) == 16 and math.isfinite(solver[9]) and solver[9] >= 0,
                        'invalid scored first-solver timer')
                times[arm].append(solver[9])
        for arm, diagnostics in selected.items():
            require(len(diagnostics) == 3, 'incomplete first-solver diagnostics')
            classifications[arm] = []
            for row in diagnostics:
                solver = row['solver_rows'][0]
                require(len(solver) == 16 and math.isfinite(solver[2]) and solver[2] >= 0
                        and int(solver[2]) == solver[2], 'invalid diagnostic first-solver iteration count')
                classifications[arm].append(engine_projection_uses_cells(
                    row['phase_diagnostics']['solver_trace'], arm))
        require(all(len(set(values)) == 1 for values in classifications.values()),
                'engine cell-use decision changed across diagnostics')
        s_iterations = [r['solver_rows'][0][2] for r in selected['cells_s']]
        require(len(set(s_iterations)) == 1, 'S first-projection iterations changed across diagnostics')
        reference, candidate = [statistics.median(times[arm]) for arm in ['cells_s', 'cells_e_final']]
        cell_using = any(values[0] for values in classifications.values())
        strong = cell_using and cell['threads'] == 4 and s_iterations[0] >= 32
        bound = .8 * reference if strong else reference + max(.05 * reference, .0003)
        timers.append({'cell': cid, 'cell_using': cell_using, 's_first_projection_iterations': s_iterations[0],
                       'required_20pct_gain': strong, 'per_fit_seconds': times,
                       'timer_source': 'twelve untraced measure-phase first solver rows per arm',
                       's_median_seconds': reference, 'finalist_median_seconds': candidate,
                       'bound_seconds': bound, 'passed': not cell_using or candidate <= bound,
                       'native_minus_first_solver_seconds': {arm: [r['seconds'] - r['solver_rows'][0][9]
                           for r in measurements] for arm, measurements in scored.items()}})
    counts = {name: {'material_gains': sum(v['material_gain'] for v in values),
                     'regressions': sum(v['regression'] for v in values)} for name, values in comparisons.items()}
    passed = (counts['finalist_vs_auto']['material_gains'] >= 18
              and counts['finalist_vs_s']['material_gains'] >= 3
              and all(v['regressions'] == 0 for v in counts.values()) and all(v['passed'] for v in timers))
    return {'kind': ENGINE_KIND + '_gate', 'status': 'eligible' if passed else 'ineligible',
            'finalist': 'cells_e_final', 'plan_sha256': routing.identity(plan),
            'science_sha256': routing.identity(science_receipt), 'measurement_sha256': routing.identity(measurement),
            'comparisons': comparisons, 'counts': counts, 'first_solver_comparisons': timers,
            'exact_ownership_science': 'passed', 'extensions': 'none', 'exclusions': [],
            'non_solver_difference': 'diagnostic only; native minus first-solver time is not an additive phase decomposition'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest='command', required=True)
    prep = subs.add_parser('prepare')
    for name in ['root', 'generator', 'harness', 'plugin']:
        prep.add_argument('--' + name, type=Path, required=True)
    factor = subs.add_parser('prepare-factor', help='reuse a frozen four-arm parent; prepare only the D_f request')
    deferred = subs.add_parser('prepare-deferred',
                               help='reuse the frozen original four-arm parent; prepare only the deferred request')
    membership = subs.add_parser('prepare-membership',
                                 help='reuse the frozen original parent; prepare the two membership requests')
    occurrence = subs.add_parser('prepare-occurrence')
    occurrence.add_argument('--diagnostic-admission', type=Path, required=True)
    admission = subs.add_parser('prepare-admission')
    admission.add_argument('--prior-occurrence-plan', type=Path, required=True)
    engine = subs.add_parser('prepare-engine')
    engine.add_argument('--prior-admission-plan', type=Path, required=True)
    for sub in [factor, deferred, membership, occurrence, admission, engine]:
        for name in ['root', 'parent-plan', 'harness', 'plugin']:
            sub.add_argument('--' + name, type=Path, required=True)
    for mode in ['science', 'smoke', 'measure']:
        sub = subs.add_parser(mode)
        sub.add_argument('--plan', type=Path, required=True)
        sub.add_argument('--output', type=Path, required=True)
        if mode == 'measure':
            sub.add_argument('--science', type=Path, required=True)
        else:
            sub.add_argument('--confirmation-plan', type=Path)
            sub.add_argument('--confirmation-driver', type=Path)
    gate = subs.add_parser('membership-gate', help='evaluate retained D1 evidence for the fixed M finalist')
    engine_gate_parser = subs.add_parser('engine-gate', help='evaluate the fixed D4 A+B finalist')
    for gate_parser in [gate, engine_gate_parser]:
        for name in ['plan', 'science', 'measurement', 'output']:
            gate_parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if args.command in {'membership-gate', 'engine-gate'}:
        gate_function = engine_gate if args.command == 'engine-gate' else membership_gate
        result = gate_function(*(routing.unseal(read(path)) for path in
                                   [args.plan, args.science, args.measurement]))
        save(args.output, routing.seal(result))
        require(result['status'] == 'eligible', 'membership discovery gate did not pass')
    elif args.command == 'prepare':
        prepare(args.root, args.generator.resolve(strict=True), args.harness.resolve(strict=True),
                args.plugin.resolve(strict=True))
    elif args.command == 'prepare-factor':
        prepare_factor(args.root, args.parent_plan, args.harness, args.plugin)
    elif args.command == 'prepare-deferred':
        prepare_deferred(args.root, args.parent_plan, args.harness, args.plugin)
    elif args.command == 'prepare-engine':
        prepare_engine(args.root, args.parent_plan, args.harness, args.plugin, args.prior_admission_plan)
    elif args.command == 'prepare-admission':
        prepare_admission(args.root, args.parent_plan, args.harness, args.plugin, args.prior_occurrence_plan)
    elif args.command == 'prepare-occurrence':
        prepare_occurrence(args.root, args.parent_plan, args.harness, args.plugin, args.diagnostic_admission)
    elif args.command == 'prepare-membership':
        prepare_membership(args.root, args.parent_plan, args.harness, args.plugin)
    else:
        plan = routing.unseal(read(args.plan))
        result = (measure(plan, routing.unseal(read(args.science)), args.output) if args.command == 'measure'
                  else science(plan, args.output, smoke=args.command == 'smoke',
                               confirmation_plan=args.confirmation_plan, confirmation_driver=args.confirmation_driver))
        require(result['status'] in {'passed', 'complete'}, result.get('failure') or 'campaign failed')
        if 'parent_control_status' in result:
            count = (f"comparisons={result['parent_control_count']} calls={result['parent_control_native_calls']}"
                     if plan['kind'] in PAIRED_CONTROL_KINDS else f"calls={result['parent_control_count']}")
            print(f"COMPACT_CELLS_PARENT_CONTROLS status={result['parent_control_status']} {count}",
                  flush=True)
    print(f'COMPACT_CELLS_SUCCESS stage={args.command}', flush=True)


if __name__ == '__main__':
    main()
