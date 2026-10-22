#!/usr/bin/env python3
"""Frozen remaining-cost calibration, using qualify's native/science runner.

This tool retains labels; it neither selects coefficients nor activates Auto.
The two recorded calls per arm are one mirrored block, not two blocks.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re

import routing
from native_io import case_header

MODE = 'two-fe-remaining-calibration-v1'
SEEDS = (2026102201, 2026103201)
FAMILIES = ('dense', 'weak', 'high-degree-weak', 'repeated-cells',
            'disconnected', 'weight-dynamic', 'wide-rhs', 'inactive-columns')
ARMS = dict(zip(('never', 'force16', 'force32'), routing.REMAINING[1:]))
CASE_IDS = tuple(f's{seed}-{family}-{memory}' for seed in SEEDS
                 for family in FAMILIES for memory in ('fast', 'low'))
ACTION = ('role', 'start', 'end', 'total', 'block', 'r', 'stride', 'tolerance_bits', 'final')
CHECKPOINT = ('epoch', 'local', 'remaining', 'n', 'p', 'q', 'd', 'workers', 'weighted',
              'scaled', 'forecast_h', 'forecast_d', 'classes', 'history_width',
              'history_actions', 'history_steps', 'history_live', 'history_original',
              'stamp_ns', 'state', 'build')
END = ('steps', 'live', 'elapsed_ns', 'build_ns', 'restart_ns', 'startup_ns',
       'continuation_ns', 'installed')
OPTIONAL = {'forecast_h', 'forecast_d', 'classes', 'history_width', 'history_actions',
            'history_steps', 'history_live', 'history_original', 'state', 'steps', 'live',
              'build_ns', 'restart_ns', 'startup_ns', 'continuation_ns'}


def streams(seed, family):
    offsets = (0, 1) if family in ('disconnected', 'wide-rhs', 'inactive-columns') else (
        (0, 17) if family == 'weight-dynamic' else (0,))
    return [seed + offset for offset in offsets]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fixed(platform):
    require(platform in ('Darwin', 'Linux'), 'unsupported calibration platform')
    return dict(diagnostic=MODE, scope='two', coverage='smoke', threads=[1, 4, 8],
                blocks=1, shards=1, replicas=1, seed=2026102204,
                timeout_s=120, budget_s=14400)


def validate(value):
    require(all(value.get(k) == v for k, v in fixed(value['platform']).items()),
            'frozen calibration schedule changed')
    require(not any(k in value for k in ('continuation', 'recovery', 'min_distinct_hosts_per_cell')),
            'calibration has no resampling or replacement observations')
    require(value.get('warmups', 1) == 1, 'calibration warmup changed')
    require(set(value['artifacts']) == set(ARMS), 'calibration requires three exact controls')
    sources = set()
    for role, request in ARMS.items():
        artifact = value['artifacts'][role]
        require(artifact['request'] == request and artifact['families'] == ['two'],
                'calibration control/request scope changed')
        require(re.fullmatch('[0-9a-f]{40}', artifact['revision']) is not None,
                'calibration requires a complete source revision')
        sources.add((artifact['revision'], artifact['build']))
    require(len(sources) == 1, 'controls require the same source and build')
    if 'files' in value:
        require(len({value['files'][a['path']] for a in value['artifacts'].values()}) == 1,
                'controls require the same actual binary')
    require([c['id'] for c in value['cases']] == list(CASE_IDS),
            'calibration requires its 32 ordered input/memory cases')
    for case in value['cases']:
        require(case['family'] in FAMILIES and case['memory_limit_bytes'] is None
                and case['oracle']['kind'] in ('routing', 'inactive'),
                'calibration needs independent numerical references')
    require(len(value['cases']) * len(value['threads']) * len(ARMS) * (1 + 2 * value['blocks']) == 864,
            'calibration call cap changed')


def validate_case(source, item, *, mode=MODE):
    import qualify as q
    require(mode == MODE, 'wrong calibration mode')
    meta = q.read(source.with_suffix('.json'))
    require(meta.get('seed') in SEEDS and meta.get('family') == item['family']
            and meta.get('small') is not True, 'unbound calibration input')
    require(meta.get('random_streams') == streams(meta['seed'], item['family'])
            and routing.sha(meta.get('generator_sha256')), 'missing generated-stream provenance')
    with source.open('rb') as stream:
        _, args = case_header(stream)
    fields = dict(a.decode().split('=', 1) for a in args[1:])
    memory = fields.get('memory')
    require(memory in ('fast', 'low') and fields.get('solver') == fields.get('preconditioner') == 'auto',
            'calibration requires explicit memory mode and Auto/Auto')
    require(item['id'] == f"s{meta['seed']}-{item['family']}-{memory}", 'calibration identity mismatch')
    inspected = q.routing_cases.inspect_case(source, item['family'])
    require(inspected['absorbed_terms'] == 2 and inspected['categorical_intercepts'],
            'calibration requires two categorical intercepts')
    require(meta['body_sha256'] == inspected['body_sha256'], 'calibration body changed')


def validate_panel(value, manifest_path):
    import qualify as q
    grouped = {}
    for item in value['cases']:
        meta = q.read(q.resolve(manifest_path, item['metadata']))
        key = (meta['seed'], item['family'])
        grouped.setdefault(key, []).append(meta['body_sha256'])
    require(len(grouped) == 16 and all(len(hashes) == 2 and hashes[0] == hashes[1]
                                     for hashes in grouped.values()),
            'Fast/Low must share exactly the same generated body')
    require(len({hashes[0] for hashes in grouped.values()}) == 16,
            'generated calibration bodies collide')
    stages = [{stream for family in FAMILIES for stream in streams(seed, family)}
              for seed in (*SEEDS, 2026104201, 2026105201)]
    require(all(not left & right for index, left in enumerate(stages)
                for right in stages[index+1:]), 'calibration and held-out random streams collide')


def comparisons(value):
    validate(value)
    return (('never', 'force16'), ('never', 'force32'))


def compare_science(value, meta, snapshots):
    import routing_science as science
    validate(value)
    for _, role in comparisons(value):
        reference, candidate = snapshots['never'], snapshots[role]
        science.compatibility(reference[0], candidate[0], reference[1], candidate[1], meta, 'auto',
                              selected_policy=routing.policy(routing.PORTFOLIO_UNSEEDED))


def parse_labels(log):
    """Strict scalar schema; keep censored/unknown forecasts as None, never zero."""
    actions, checkpoints, seen = [], [], set()
    for line in log.splitlines():
        if not line.startswith('FEREG_POLICY_LABEL|'):
            continue
        parts = line.split('|')
        kind = parts[1]
        require(kind in ('checkpoint', 'action_end'), 'unknown policy label')
        fields = {}
        for part in parts[2:]:
            key, value = part.split('=', 1)
            require(key not in fields, 'duplicate policy field')
            if key == 'role':
                require(value in ('packed', 'outcome', 'regressor', 'final', 'oneoff'), 'invalid action role')
            elif value == 'none':
                require(key in OPTIONAL, 'missing required policy field')
                value = None
            else:
                require(re.fullmatch('[0-9]+', value) is not None, 'invalid policy integer')
                value = int(value)
                require(value <= 2**128-1, 'policy integer overflow')
            fields[key] = value
        require(set(fields) == set(ACTION + (CHECKPOINT if kind == 'checkpoint' else END)),
                'policy label schema changed')
        key = tuple(fields[k] for k in ACTION)
        require(fields['r'] > 0 and fields['stride'] >= fields['r'] and fields['final'] in (0, 1),
                'invalid projection shape')
        if kind == 'checkpoint':
            require(fields['local'] >= 16 and fields['local'] % 8 == 0,
                    'decision checkpoint cadence changed')
            require(fields['epoch'] in (0, 1) and fields['workers'] > 0
                    and all(fields[k] in (0, 1) for k in ('weighted', 'scaled', 'build')),
                    'invalid policy checkpoint')
            if checkpoints:
                prior = checkpoints[-1]
                require(key == tuple(prior[k] for k in ACTION) and fields['stamp_ns'] >= prior['stamp_ns']
                        and (fields['epoch'], fields['local']) > (prior['epoch'], prior['local']),
                        'unordered or mismatched checkpoints')
            checkpoints.append(fields)
            continue
        require(key not in seen, 'projection completed twice')
        seen.add(key)
        require(all(key == tuple(c[k] for k in ACTION) and c['stamp_ns'] <= fields['elapsed_ns']
                    for c in checkpoints), 'checkpoint outside its action')
        grants = [c for c in checkpoints if c['build']]
        require(len(grants) <= 1 and fields['installed'] in (0, 1), 'multiple action interventions')
        if fields['installed']:
            require(len(grants) == 1 and all(fields[k] is not None for k in
                    ('build_ns', 'restart_ns', 'startup_ns', 'continuation_ns')),
                    'installed action lacks disjoint economic intervals')
            require(sum(fields[k] for k in ('build_ns', 'restart_ns', 'startup_ns', 'continuation_ns'))
                    == fields['elapsed_ns'] - grants[0]['stamp_ns'], 'economic partition does not close')
        elif fields['build_ns'] is not None:
            require(len(grants) == 1 and fields['restart_ns'] is fields['startup_ns'] is fields['continuation_ns'] is None,
                    'refusal contains an installed continuation')
        else:
            require(not grants, 'intervention missing its build outcome')
        actions.append({'end': fields, 'checkpoints': checkpoints})
        checkpoints = []
    require(not checkpoints, 'incomplete action labels')
    return actions


def resource_diagnostics(log, role, revision, *, family='two'):
    import routing_science as science
    require(role in ARMS and family == 'two', 'invalid calibration control')
    result = science.resource_diagnostics(log, ARMS[role], False, {})
    actions = parse_labels(log)
    require(len(actions) == len(result['action_resources']), 'missing complete projection labels')
    expected_age = {'never': None, 'force16': 16, 'force32': 32}[role]
    for action in actions:
        for point in action['checkpoints']:
            require(point['state'] is not None, 'calibration lacks state fingerprint')
            require(not point['build'] or point['local'] == expected_age, 'forced intervention age changed')
    result['policy_actions'] = actions
    return result


def prepare(config, output):
    import qualify as q
    value = q.read(config)
    value.update(fixed(value['platform']))
    validate(value)
    return q.plan(config, output, configuration=value)


def main():
    import qualify as q
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('prepare', 'run', 'report'):
        command = sub.add_parser(name)
        command.add_argument('input', type=Path)
        command.add_argument('--output', type=Path, required=True)
        if name == 'report':
            command.add_argument('--accounting', type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        value = prepare(args.input, args.output)
        print('calibration plan sha256=' + routing.identity(value))
        return 0
    validate(q.read(args.input))
    if args.command == 'run':
        return int(q.run(args.input, args.output)['status'] != 'complete')
    return int(q.report(args.input, args.output, args.accounting)['status'] != 'diagnostic_complete')


if __name__ == '__main__':
    raise SystemExit(main())
