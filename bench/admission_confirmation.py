#!/usr/bin/env python3
"""Prepare the frozen, untouched early-refusal C3 panel; never run a candidate.

The fixed S2 finalist requires the complete D3 gate before any C3 native call.
This entrypoint only generates inputs and independent bounded witnesses. A
failed witness stops preparation without substitutes, resizing, or resampling.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
from pathlib import Path
import random
import time

import compact_cells as campaign

KIND = 'compact_cells_admission_confirmation_v1'
ARMS = ['baseline', 'cells_s2']
CASES = [
    {'family': 'short2', 'generator': 'synthetic', 'shape': 'short', 'workers': 3000, 'firms': 300, 'periods': 2},
    {'family': 'long40', 'generator': 'synthetic', 'shape': 'long', 'workers': 3000, 'firms': 300, 'periods': 40},
    {'family': 'weak40', 'generator': 'synthetic', 'shape': 'weak', 'workers': 3000, 'firms': 300, 'periods': 40},
    {'family': 'hubs', 'generator': 'high_degree_case', 'shape': 'd60-stress', 'workers': 1500, 'firms': 300, 'periods': 64},
    {'family': 'dense', 'generator': 'high_degree_case', 'shape': 'd30-mixed', 'workers': 1500, 'firms': 300, 'periods': 32},
    {'family': 'repeated', 'generator': 'synthetic', 'shape': 'long', 'workers': 1500, 'firms': 300, 'periods': 20},
    {'family': 'wide', 'generator': 'additional_case', 'shape': 'wide', 'workers': 1500, 'firms': 300, 'periods': 20},
    {'family': 'low', 'generator': 'additional_case', 'shape': 'wide', 'workers': 1500, 'firms': 300, 'periods': 20},
]
LIMITS = {**campaign.LIMITS, 'seed': 2026101733, 'data_seed': 2026101734, 'blocks': 12,
          'inference': 'all 16 settings must pass the frozen one-sided median allowance-excess bound'}
ANALYSIS = {
    'reference': 'baseline', 'finalist': 'cells_s2',
    'block_reference': 'geometric mean of two complete Auto solves',
    'block_candidate': 'geometric mean of two complete S2 solves',
    'block_excess': 'E = N - B - max(0.05 * B, 0.001)', 'block_ratio': 'N / B',
    'independent_unit': 'one complete ABBA/BAAB block within one setting',
    'blocks_per_setting': 12, 'upper_tail_alpha': .025, 'upper_median_order_statistic': 10,
    'pass_rule': 'E_(10) <= 0 in every one of the 16 settings; ties pass',
    'missing_or_invalid_rule': 'any missing block, invalid call, timeout or scientific failure fails confirmation',
    'assumptions': 'independent block scores with a common median within each setting; coverage is conditional on these assumptions',
    'exclusions': 'none', 'extensions': 'none', 'family_drops': 'none', 'alternate_finalists': 'none',
    'scope': 'isolated SCC Linux confirmation on a different host from D3; no Mac performance claim',
    'independence_caveat': 'block-order bound is conditional; within-host dependence limits inference and a second host does not make blocks independent',
}
EXECUTION = {
    'warmup_calls': 32, 'scientific_calls': 64, 'scored_calls': 768,
    'warmups_per_arm_setting': 1, 'scientific_repeats_per_arm_setting': 2,
    'scientific_checks': 'independent oracle, fixed-worker exact repeats, cross-worker scientific and exact model-discrete agreement',
    'candidate_prerequisite': 'complete D3 admission gate eligible for cells_s2',
    'freeze_order': 'this complete C3 plan must be frozen before the first D3 candidate call',
    'stopping': 'first failure or exhausted budget stops; no retries or replacements',
}
ORACLE_CAPS = {name: getattr(campaign.routing_oracle, name) for name in
               ['MAX_ROWS', 'MAX_RETAINED', 'MAX_X', 'MAX_FE', 'MAX_BYTES', 'MAX_SCHUR_WORK', 'MAX_WEIGHT_RATIO']}


def tool_files():
    return sorted({*campaign.tool_files(), Path(__file__).resolve(),
                   Path(__file__).with_name('tests') / 'test_admission_confirmation.py'})


def schedule(cell_ids):
    """Twelve shuffled complete blocks; six ABBA and six BAAB per setting."""
    rng = random.Random(LIMITS['seed'])
    orders = {cid: rng.sample([False] * 6 + [True] * 6, 12) for cid in cell_ids}
    blocks = []
    for block in range(LIMITS['blocks']):
        rows = []
        for cid in rng.sample(list(cell_ids), len(cell_ids)):
            first, second = reversed(ARMS) if orders[cid][block] else ARMS
            rows.append({'cell': cid, 'arms': [first, second, second, first]})
        blocks.append(rows)
    return blocks


def columns(spec):
    seed = LIMITS['data_seed'] + CASES.index(spec)
    values = getattr(campaign.cases, spec['generator'])(
        spec['shape'], spec['workers'], spec['firms'], spec['periods'], seed)
    if spec['family'] == 'repeated':
        values = [campaign.np.repeat(value, 4) for value in values]
    campaign.require(len(values[0]) <= ORACLE_CAPS['MAX_ROWS'] and len(values) - 3 <= ORACLE_CAPS['MAX_X'],
                     'confirmation exceeds unchanged oracle shape caps')
    return values


def validate_contract(plan):
    campaign.require(plan['kind'] == KIND and plan['arms'] == ARMS and plan['cases'] == CASES
                     and plan['limits'] == LIMITS and plan['oracle_caps'] == ORACLE_CAPS
                     and plan['library_limits'] == campaign.LIBRARY_LIMITS
                     and plan['analysis'] == ANALYSIS and plan['execution'] == EXECUTION,
                     'confirmation contract changed')
    expected = [f"{spec['family']}-t{workers}" for spec in CASES for workers in LIMITS['workers']]
    campaign.require([cell['id'] for cell in plan['cells']] == expected
                     and plan['blocks'] == schedule(expected), 'confirmation coverage/order changed')
    for cell in plan['cells']:
        campaign.require(cell['id'] == f"{cell['family']}-t{cell['threads']}"
                         and set(cell['inputs']) == set(ARMS), 'confirmation arm/cell coverage changed')
    campaign.require(type(plan['prepared_wall_ns']) is int and plan['prepared_wall_ns'] > 0,
                     'confirmation freeze time missing')


def validate(plan):
    validate_contract(plan)
    discovery = campaign.routing.unseal(campaign.read(campaign.checked(plan['discovery_plan'])))
    campaign.require(discovery['kind'] == campaign.ADMISSION_KIND
                     and campaign.routing.identity(discovery) == plan['discovery_plan_sha256'],
                     'confirmation discovery binding changed')
    campaign.validate(discovery)
    campaign.require(plan['native_affinity_cores'] == discovery['native_affinity_cores'], 'confirmation affinity changed')
    for key in ['generator', 'harness', 'plugin']:
        campaign.require(plan[key] == discovery[key], 'confirmation artifact differs from discovery')
    for path, sha in plan['frozen_files'].items():
        campaign.checked({'path': path, 'sha256': sha})
    bindings = [plan['discovery_plan'], *(plan[key] for key in ['generator', 'harness', 'plugin'])]
    for path in tool_files():
        campaign.require(plan['frozen_files'].get(str(path.resolve())) == campaign.digest(path),
                         'unbound confirmation tool')
    for cell in plan['cells']:
        bindings.extend([cell['source'], cell['metadata'], cell['witness'], *cell['inputs'].values()])
        meta = campaign.read(campaign.checked(cell['metadata']))
        campaign.require(cell['source']['sha256'] == meta['case_sha256'], 'confirmation source binding changed')
        _, witness_bindings = campaign.routing_oracle.load_reference(campaign.checked(cell['witness']),
                                                                   meta['case_sha256'], meta['body_sha256'])
        bindings.extend({'path': str(path.resolve()), 'sha256': sha} for path, sha in witness_bindings.items())
        for item in cell['inputs'].values():
            campaign.require(campaign.routing_cases.inspect_case(campaign.checked(item))['body_sha256']
                             == meta['body_sha256'], 'confirmation request changed data/model body')
    for item in bindings:
        campaign.require(plan['frozen_files'].get(item['path']) == item['sha256'],
                         'unbound confirmation dependency')


def prepare(root, discovery_path):
    discovery_binding = campaign.ref(discovery_path)
    discovery = campaign.routing.unseal(campaign.read(discovery_path))
    campaign.require(discovery['kind'] == campaign.ADMISSION_KIND,
                     'confirmation requires the membership discovery plan')
    campaign.validate(discovery)
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=False)
    frozen = [discovery_binding, *(discovery[key] for key in ['generator', 'harness', 'plugin']),
              *(campaign.ref(path) for path in tool_files())]
    cells = []
    for spec in CASES:
        family = spec['family']
        source = root / (family + '.case')
        values = columns(spec)
        campaign.cases.write_case(Path(discovery['generator']['path']), source, values)
        if family == 'low':
            normal = source.with_suffix('.normal.case')
            source.rename(normal)
            campaign.routing_cases.copy_import(normal, source, low=True)
            normal.unlink()
        meta = campaign.routing_cases.inspect_case(source, family)
        meta.update(family=family, case_sha256=campaign.digest(source),
                    **campaign.cases.graph_summary(values))
        metadata = source.with_suffix('.json')
        campaign.save(metadata, meta)
        witness = root / (family + '.oracle.json')
        campaign.routing_oracle.write_reference(source, witness)
        _, bindings = campaign.routing_oracle.load_reference(witness, meta['case_sha256'], meta['body_sha256'])
        frozen.extend(campaign.ref(path) for path in [source, metadata, *bindings])
        for workers in LIMITS['workers']:
            cid = f'{family}-t{workers}'
            inputs = {}
            for arm in ARMS:
                target = root / f'{cid}-{arm}.case'
                campaign.routing.prepare(source, target, arm, workers)
                campaign.require(campaign.routing_cases.inspect_case(target)['body_sha256'] == meta['body_sha256'],
                                 'confirmation request changed data/model body')
                inputs[arm] = campaign.ref(target)
                frozen.append(inputs[arm])
            cells.append({'id': cid, 'family': family, 'threads': workers, 'source': campaign.ref(source),
                          'metadata': campaign.ref(metadata), 'witness': campaign.ref(witness), 'inputs': inputs})
    plan = {'kind': KIND, 'arms': ARMS, 'cases': CASES, 'limits': LIMITS, 'oracle_caps': ORACLE_CAPS,
            'library_limits': campaign.LIBRARY_LIMITS, 'analysis': ANALYSIS, 'execution': EXECUTION,
            'discovery_plan': discovery_binding, 'native_affinity_cores': discovery['native_affinity_cores'],
            'discovery_plan_sha256': campaign.routing.identity(discovery),
            **{key: discovery[key] for key in ['generator', 'harness', 'plugin']},
            'cells': cells, 'blocks': schedule([cell['id'] for cell in cells]),
            'frozen_files': {item['path']: item['sha256'] for item in frozen},
            'prepared_wall_ns': time.time_ns(), 'created_utc': datetime.now(timezone.utc).isoformat(),
            'root': str(root)}
    validate(plan)
    campaign.save(root / 'plan.json', campaign.routing.seal(plan))
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--discovery-plan', type=Path, required=True)
    args = parser.parse_args()
    plan = prepare(args.root, args.discovery_plan)
    print('ADMISSION_CONFIRMATION_PREPARED sha256=' + campaign.routing.identity(plan), flush=True)


if __name__ == '__main__':
    main()
