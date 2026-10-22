#!/usr/bin/env python3
"""One frozen complete-command development comparison; never qualification.

Uses qualify's native runner, independent science, repeats, host observation,
resource gates and complete-call timer. No fitting, retries or extra labels.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re

import routing
import two_fe_policy as calibration
from native_io import ROOT, case_header, digest

MODE = 'two-fe-gates-development-v1'
SEEDS = (2026104201,)
FAMILIES = calibration.FAMILIES
CASE_IDS = tuple(f's{SEEDS[0]}-{family}-{memory}' for family in FAMILIES
                 for memory in ('fast', 'low'))
ORIGINAL = 'f35f3a4dbb306bd4c4f5f7d46a406be892bd0749'
INCUMBENT = '1cdb225e2460e391ad38f856755ae3ea5efb2e5b'
ARMS = {'original': 'auto', 'incumbent': 'auto', 'candidate': routing.GATES}
FIT_SHA256 = '5b42c6e24862e0a4c008aa9a1b7d0256e8d335e2ac82d50d9f530a73b2bff573'
WIRE_SHA256 = '4acee95822a5083b42f532f08a4b5d1a29979b0b278af1b8287540e1e506c38c'
require = calibration.require
streams = calibration.streams


def selected_binding():
    fit = ROOT / 'bench/results/two-fe-policy-gates-fit-20261022.json'
    require(digest(fit) == FIT_SHA256, 'selected fit changed')
    result = routing.read_json(fit)['result']
    require(result['gates'] == [dict(kind=2, feature=16, cut=0, build_if_le=1),
                               dict(kind=0, feature=0, cut=0, build_if_le=0)], 'selected gates changed')
    return dict(fit=str(fit), spec=str(ROOT / 'docs/two_fe_auto_gates.md'),
                fit_sha256=FIT_SHA256, wire_hex=result['encoded_hex'], wire_sha256=WIRE_SHA256)


def fixed(platform):
    require(platform in ('Darwin', 'Linux'), 'unsupported development platform')
    return dict(diagnostic=MODE, scope='two', coverage='smoke', threads=[1, 4, 8],
                blocks=1, shards=1, replicas=1, seed=2026102205,
                timeout_s=120, budget_s=14400)


def validate(value):
    require(all(value.get(k) == v for k, v in fixed(value['platform']).items()),
            'frozen development schedule changed')
    require(not any(k in value for k in ('continuation', 'recovery', 'min_distinct_hosts_per_cell')),
            'development has no retries, replacements or continuation')
    require(value.get('warmups', 1) == 1, 'development warmup changed')
    binding = value.get('policy_binding', {})
    require(set(binding) == {'fit', 'spec', 'fit_sha256', 'wire_hex', 'wire_sha256'}
            and binding['fit_sha256'] == FIT_SHA256 and binding['wire_sha256'] == WIRE_SHA256,
            'development selected-model binding changed')
    require(isinstance(binding['wire_hex'], str) and re.fullmatch('[0-9a-f]{256}', binding['wire_hex'])
            and hashlib.sha256(bytes.fromhex(binding['wire_hex'])).hexdigest() == WIRE_SHA256,
            'development selected wire changed')
    require(set(value['artifacts']) == set(ARMS), 'development requires three protected arms')
    for role, request in ARMS.items():
        artifact = value['artifacts'][role]
        families = [] if role == 'original' else ['many'] if role == 'incumbent' else ['many', 'two']
        require(artifact['request'] == request and artifact['families'] == families,
                'development arm/request scope changed')
        require(re.fullmatch('[0-9a-f]{40}', artifact['revision']) is not None,
                'development requires complete source revisions')
    require(value['artifacts']['original']['revision'] == ORIGINAL
            and value['artifacts']['incumbent']['revision'] == INCUMBENT,
            'development public baselines changed')
    if 'files' in value:
        require(value['files'].get(binding['fit']) == FIT_SHA256
                and routing.sha(value['files'].get(binding['spec'])), 'development fit/spec file binding missing')
        identities = {(value['files'][a['path']], a['request']) for a in value['artifacts'].values()}
        require(len(identities) == 3, 'development arms cannot alias away planned calls')
    require([c['id'] for c in value['cases']] == list(CASE_IDS),
            'development requires its 16 ordered input/memory cases')
    for case in value['cases']:
        require(case['family'] in FAMILIES and case['memory_limit_bytes'] is None
                and case['oracle']['kind'] == ('inactive' if case['family'] == 'inactive-columns' else 'routing'),
                'development needs its independent numerical references')
    require(len(value['cases']) * len(value['threads']) * len(ARMS) * (1 + 2 * value['blocks']) == 432,
            'development call cap changed')


def validate_case(source, item, *, mode=MODE):
    import qualify as q
    require(mode == MODE, 'wrong development mode')
    meta = q.read(source.with_suffix('.json'))
    require(meta.get('seed') == SEEDS[0] and meta.get('family') == item['family']
            and meta.get('small') is False, 'unbound held-out development input')
    require(meta.get('random_streams') == streams(SEEDS[0], item['family'])
            and routing.sha(meta.get('generator_sha256'))
            and routing.sha(meta.get('generator_sources_sha256')), 'missing generated-stream provenance')
    with source.open('rb') as stream:
        _, args = case_header(stream)
    fields = dict(a.decode().split('=', 1) for a in args[1:])
    memory = fields.get('memory')
    require(memory in ('fast', 'low') and fields.get('solver') == fields.get('preconditioner') == 'auto',
            'development requires explicit memory mode and Auto/Auto')
    require(item['id'] == f"s{SEEDS[0]}-{item['family']}-{memory}", 'development identity mismatch')
    inspected = q.routing_cases.inspect_case(source, item['family'])
    require(inspected['absorbed_terms'] == 2 and inspected['categorical_intercepts']
            and meta['body_sha256'] == inspected['body_sha256'], 'development model/body changed')


def validate_panel(value, manifest_path):
    import qualify as q
    grouped = {}
    for item in value['cases']:
        meta = q.read(q.resolve(manifest_path, item['metadata']))
        require(meta['seed'] == SEEDS[0], 'held-out development seed changed')
        grouped.setdefault(item['family'], []).append(meta['body_sha256'])
    require(set(grouped) == set(FAMILIES) and all(len(h) == 2 and h[0] == h[1] for h in grouped.values()),
            'Fast/Low must share exactly eight generated bodies')
    require(len({h[0] for h in grouped.values()}) == 8, 'development bodies collide')
    from two_fe_policy_inputs import stream_plan
    stream_plan()  # Verify calibration, development and final streams are disjoint.


def comparisons(value):
    validate(value)
    return (('original', 'candidate'), ('incumbent', 'candidate'))


def compare_science(value, meta, snapshots):
    import routing_science as science
    for role, target in comparisons(value):
        before, after = snapshots[role], snapshots[target]
        science.compatibility(before[0], after[0], before[1], after[1], meta, 'auto',
                              selected_policy=routing.policy(routing.PORTFOLIO_UNSEEDED))


def gate_events(log):
    events, closed, pending_build = [], False, False
    action_events, action_ordinal = [], 0
    for line in log.splitlines():
        require(not line.startswith('FEREG_POLICY_LABEL|'), 'selected structural gate used historical observer labels')
        if line.startswith('FEREG_SOLVER_TRACE|phase=routing_action|'):
            pairs = [part.split('=', 1) for part in line.split('|')[1:]]
            require(all(len(p) == 2 for p in pairs) and len({p[0] for p in pairs}) == len(pairs),
                    'malformed gate action trace')
            action = dict(pairs)
            require(not pending_build, 'gate grant crosses an action boundary')
            for event in action_events:
                if event['event'] == 'decision':
                    require(int(action['iterations']) >= 16, 'gate decision exceeds completed action age')
                    role = event['role']
                    require(role == 'Unknown' or action.get('final') == str(int(role == 'Final')),
                            'gate role disagrees with completed action')
                event['action_ordinal'] = action_ordinal
            action_events = []
            action_ordinal += 1
            continue
        if line.startswith('FEREG_SOLVER_TRACE|phase=auto_schur_route|'):
            pairs = [part.split('=', 1) for part in line.split('|')[1:]]
            require(all(len(p) == 2 for p in pairs) and len({p[0] for p in pairs}) == len(pairs),
                    'malformed gate build trace')
            route = dict(pairs)
            require(pending_build and route.get('iterations') == '16'
                    and route.get('installed') in ('0', '1')
                    and route.get('refusal') == ('None' if route['installed'] == '1' else 'Some(MemoryBudget)'),
                    'gate build has no grant, wrong age or invalid refusal')
            pending_build = False
            continue
        if not line.startswith('FEREG_SOLVER_TRACE|phase=two_fe_gates|'):
            continue
        pairs = [part.split('=', 1) for part in line.split('|')[1:]]
        require(all(len(p) == 2 for p in pairs) and len({p[0] for p in pairs}) == len(pairs),
                'malformed gate trace')
        event = dict(pairs)
        common = {'phase', 'event', 'grant', 'terminal', 'proxy_entries', 'bank_bytes'}
        require(common <= set(event) and not pending_build, 'incomplete gate event or missing build outcome')
        require(event['terminal'] == '1' and event['proxy_entries'] == event['bank_bytes'] == '0',
                'selected structural gate changed its terminal/dependency contract')
        if event['event'] == 'decision':
            require(not closed and set(event) == common | {'local', 'role'} and event['local'] == '16',
                    'gate episode reopened or decision schema/age changed')
            require(event['role'] in ('Packed', 'Outcome', 'Final', 'Unknown')
                    and event['grant'] == str(int(event['role'] in ('Packed', 'Outcome'))),
                    'selected gate action/decision mismatch')
            pending_build = event['grant'] == '1'
        else:
            require(event['event'] == 'correction' and set(event) == common and event['grant'] == '0',
                    'invalid correction closure')
        closed = True
        events.append(event)
        action_events.append(event)
    require(not pending_build, 'gate grant lacks a completed build/refusal outcome')
    require(not action_events, 'gate events lack a completed action')
    return events


def semantic_signature(detail):
    """Warmup-only discrete policy evidence, excluding worker representation."""
    return routing.identity({'events': detail['gate_events'],
        'routes': [{k: route[k] for k in ('iterations', 'installed', 'refusal')}
                   for route in detail['route_trace']],
        'hierarchies': [None if h is None else {k: v for k, v in h.items()
                       if k not in ('application', 'parallel_operators', 'batch_columns')}
                       for h in detail['route_hierarchy']]})


def resource_diagnostics(log, role, revision, *, family='two'):
    import routing_science as science
    require(role == 'candidate' and family == 'two', 'unexpected development resource arm')
    result = science.resource_diagnostics(log, routing.GATES, False, {})
    result['gate_events'] = gate_events(log)
    return result


def prepare(config, output):
    import qualify as q
    value = q.read(config)
    require(set(value) == {'platform', 'artifacts', 'harness', 'cases'}, 'invalid development configuration fields')
    value.update(fixed(value['platform']))
    value['policy_binding'] = selected_binding()
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
        print('development plan sha256=' + routing.identity(value))
        return 0
    validate(q.read(args.input))
    if args.command == 'run':
        return int(q.run(args.input, args.output)['status'] != 'complete')
    return int(q.report(args.input, args.output, args.accounting)['status'] != 'diagnostic_complete')


if __name__ == '__main__':
    raise SystemExit(main())
