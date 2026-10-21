#!/usr/bin/env python3
"""Fixed CMG setup diagnostic using qualify's native/science/observation runner.

No activation qualification, automatic retry, input generation or native build.
Config: optional diagnostic mode, platform, seed (order seed), harness, artifacts and cases. Artifacts have
path/revision/build; cases use qualify's input/routing-oracle fields and need a
seed=2026101821 metadata sidecar. Paths are relative to the config file.
"""
from __future__ import annotations

import argparse
import copy
import re
from pathlib import Path

MODE = 'cmg-setup-v1'
CANDIDATE_MODE = 'cmg-setup-candidate-v1'
ACCEPTED = 'accepted_hidden'
SETUP = 'setup_hidden'
CANDIDATE_ARMS = (ACCEPTED, SETUP)
CANDIDATE_SEED = 2026102104
PRE = 'pre_refactor_enabled'
POST = 'post_refactor_hidden'
PUBLIC = 'current_public'
ARMS = (PRE, POST, PUBLIC)
REVISIONS = {PRE: 'd5b06cf369be33e04d10ea6daa819cc507387380 + DEFAULT_TWO=true',
             POST: '3f9d54eeacb2ced61bf8cb9a629daa0d1a6d23d9',
             PUBLIC: '3f9d54eeacb2ced61bf8cb9a629daa0d1a6d23d9'}
CASES = ('high-degree-weak', 'weak-low', 'repeated-cells', 'wide-rhs')
INPUT_SEED = 2026101821
HIGH_DEGREE_SHA256 = '9631343a441c33c277d3782b388a584f93c64486e2baa817e9e4bbf1b2623bb7'


def fixed(platform, mode=MODE):
    import qualify as q
    q.require(platform in ('Linux', 'Darwin'), 'unsupported diagnostic platform')
    q.require(mode in (MODE, CANDIDATE_MODE), 'unknown setup diagnostic mode')
    return {'diagnostic': mode, 'scope': 'exact' if mode == CANDIDATE_MODE else 'two', 'coverage': 'smoke',
            'threads': [1, 4, 8], 'shards': 1, 'replicas': 6 if platform == 'Linux' else 1,
            'blocks': 2 if platform == 'Linux' else 12, 'timeout_s': 120,
            'budget_s': 3600 if platform == 'Linux' else 14400}


def artifact_policy(role):
    return {'request': 'incremental-two' if role in (POST, *CANDIDATE_ARMS) else 'auto',
            'families': ['many'] if role == PUBLIC else ['many', 'two']}


def valid_revision(role, revision):
    if role == SETUP:
        return isinstance(revision, str) and re.fullmatch('[0-9a-f]{40}', revision) is not None
    return revision == REVISIONS.get(POST if role == ACCEPTED else role) and role in (*ARMS, ACCEPTED)


def validate(value):
    """Called by plan, run and report; the mode cannot loosen ordinary plans."""
    import qualify as q
    candidate = value.get('diagnostic') == CANDIDATE_MODE
    roles = CANDIDATE_ARMS if candidate else ARMS
    q.require(all(value.get(k) == v for k, v in fixed(value['platform'], value.get('diagnostic')).items()),
              'diagnostic schedule/scope changed')
    q.require(not any(k in value for k in ('continuation', 'recovery')),
              'diagnostic has no continuation or automatic recovery')
    q.require(not candidate or value.get('seed') == CANDIDATE_SEED, 'setup candidate order seed changed')
    q.require(set(value['artifacts']) == set(roles), 'diagnostic needs its named arms')
    for role in roles:
        artifact = value['artifacts'][role]
        q.require(valid_revision(role, artifact['revision']), 'diagnostic source revision/patch changed: ' + role)
        q.require(all(artifact.get(k) == v for k, v in artifact_policy(role).items()),
                  'diagnostic request/family changed: ' + role)
    q.require([c['id'] for c in value['cases']] == list(CASES), 'diagnostic needs the four ordered families')
    if 'files' in value and candidate:
        q.require(value['files'][value['artifacts'][ACCEPTED]['path']]
                  != value['files'][value['artifacts'][SETUP]['path']],
                  'setup candidate requires two distinct actual binaries for its fixed call budget')
    if 'files' in value and not candidate:
        q.require(value['files'][value['artifacts'][POST]['path']]
                  == value['files'][value['artifacts'][PUBLIC]['path']],
                  'current public and hidden must use the same actual binary')
    for item in value['cases']:
        q.require(item['family'] == item['id'] and item['memory_limit_bytes'] is None
                  and item['oracle']['kind'] == 'routing', 'diagnostic requires independent routing witnesses')


def validate_case(source, item, *, mode=MODE):
    import qualify as q
    meta = q.read(source.with_suffix('.json'))
    q.require(meta.get('seed') == INPUT_SEED and meta.get('small') is not True,
              'diagnostic requires full-size inputs with seed 2026101821')
    with source.open('rb') as stream:
        shape, args = q.case_header(stream)
    fields = dict(a.decode().split('=', 1) for a in args[1:])
    if mode == CANDIDATE_MODE:
        q.require(fields.get('solver') == 'auto' and fields.get('preconditioner') == 'auto',
                  'setup candidate source must request Auto/Auto')
    expected = (600000, 12) if item['id'] == CASES[0] else (100000, 26 if item['id'] == 'wide-rhs' else 12)
    inspected = q.routing_cases.inspect_case(source, item['family'])
    q.require(tuple(shape[:2]) == expected and inspected['absorbed_terms'] == 2
              and inspected['categorical_intercepts'], 'diagnostic shape/design changed')
    q.require((fields.get('memory') == 'low') == (item['id'] == 'weak-low'), 'diagnostic memory mode changed')
    if item['id'] == CASES[0]:
        q.require(q.digest(source) == HIGH_DEGREE_SHA256, 'historical high-degree input was not reproduced')


def comparisons(value):
    validate(value)
    if value['diagnostic'] == CANDIDATE_MODE:
        return ((ACCEPTED, SETUP),)
    return ((PRE, POST), (PUBLIC, POST))


def compare_science(value, meta, snapshots):
    """Historical scoped Auto checks or exact setup checks; no new tolerance."""
    import qualify as q
    for reference, changed in comparisons(value):
        before, after = snapshots[reference], snapshots[changed]
        if value['diagnostic'] == CANDIDATE_MODE:
            q.require(before[2]['fingerprint'] == after[2]['fingerprint'],
                      'setup candidate exact science/solver fingerprint changed')
            q.require(q.worker_signature(before[0]) == q.worker_signature(after[0]),
                      'setup candidate solver decisions changed')
            for field in ('route_trace', 'route_hierarchy'):
                q.require(field in before[2] and field in after[2],
                          'setup candidate missing ' + field)
                q.require(before[2][field] == after[2][field],
                          'setup candidate exact ' + field + ' changed')
            continue
        q.science.compatibility(before[0], after[0], before[1], after[1], meta, 'auto',
                                selected_policy=q.routing.policy(q.routing.PORTFOLIO_UNSEEDED))


LEGACY_ROUTE_FIELDS = {'phase', 'checkpoint', 'samples', 'iterations', 'setup_work',
                       'setup_limit', 'installed', 'refusal'}
LEGACY_ACTION_FIELDS = {'phase', 'preset', 'iterations', 'native_limit', 'setup_work',
                        'setup_limit', 'memory_bytes', 'memory_limit', 'final', 'certified'}
LEGACY_REFUSALS = {'Some(WorkBudget)', 'Some(MemoryBudget)', 'Some(Unused)'}


def legacy_count(fields, key):
    import qualify as q
    value = fields.get(key, '')
    q.require(value.isascii() and value.isdigit() and int(value) <= 2**64 - 1,
              'invalid historical counter: ' + key)
    return int(value)


def historical_route(fields):
    """Exact d5b route schema; hierarchy did not exist in this trace format."""
    import qualify as q
    q.require(set(fields) == LEGACY_ROUTE_FIELDS and fields['phase'] == 'auto_schur_route',
              'unexpected historical route schema')
    q.require(fields['checkpoint'] == '16' and fields['samples'] == '1'
              and legacy_count(fields, 'iterations') >= 16, 'invalid historical checkpoint/samples/iterations')
    q.require(legacy_count(fields, 'setup_work') <= legacy_count(fields, 'setup_limit'),
              'historical route setup_work exceeded its cap')
    q.require((fields['installed'] == '1' and fields['refusal'] == 'None')
              or (fields['installed'] == '0' and fields['refusal'] in LEGACY_REFUSALS),
              'historical installation/refusal mismatch')
    return None


def resource_diagnostics(log, role, revision, *, family='two'):
    """Revision-gated historical cap validation; current diagnostics stay strict."""
    import qualify as q
    q.require(valid_revision(role, revision), 'unrecognized diagnostic arm/revision')
    q.require(family in artifact_policy(role)['families'], 'family is not enabled for this diagnostic arm')
    legacy = role == PRE
    setup = []
    if legacy:
        q.require(family == 'two', 'historical schema is bound to the two-FE diagnostic')
        for line in log.splitlines():
            if not line.startswith('FEREG_SOLVER_TRACE|'):
                continue
            parts = [part.split('=', 1) for part in line.split('|')[1:]]
            if not any(part == ['phase', phase] for part in parts
                       for phase in ('auto_schur_route', 'routing_action')):
                continue
            q.require(all(len(part) == 2 for part in parts)
                      and len({part[0] for part in parts}) == len(parts), 'malformed historical trace fields')
            fields = dict(parts)
            if fields['phase'] == 'auto_schur_route':
                historical_route(fields)
            else:
                q.require(set(fields) == LEGACY_ACTION_FIELDS, 'unexpected historical action schema')
                used, limit = (legacy_count(fields, key) for key in ('setup_work', 'setup_limit'))
                q.require(used <= limit, 'historical action setup_work exceeded its cap')
                setup.append((used, limit))
    result = q.science.resource_diagnostics(log, 'incremental-' + family, False, {},
        expected_preset='routing_incremental_' + family,
        route_validator=historical_route if legacy else q.science.route_hierarchy)
    if legacy:
        q.require(len(setup) == len(result['action_resources']), 'historical action coverage changed')
        for action, (used, limit) in zip(result['action_resources'], setup):
            action.update(setup_work_used=used, setup_work_limit=limit)
        result['resource_checks']['setup_work_caps'] = 'passed'
        result['hierarchy_evidence'] = 'unavailable: pinned historical trace schema has no hierarchy metadata'
        result['trace_schema'] = 'd5b06cf-two-fe'
    return result


def prepare(config_path, output):
    import qualify as q
    config_path, output = Path(config_path).resolve(), Path(output).resolve()
    value = q.read(config_path)
    q.require(set(value) - {'diagnostic'} == {'platform', 'seed', 'harness', 'artifacts', 'cases'},
              'invalid diagnostic config fields')
    value = copy.deepcopy(value)
    value.update(fixed(value['platform'], value.get('diagnostic', MODE)))
    roles = CANDIDATE_ARMS if value['diagnostic'] == CANDIDATE_MODE else ARMS
    q.require(set(value['artifacts']) == set(roles), 'diagnostic needs its named arms')
    # The shared writer canonicalizes object keys before the frozen plan is read.
    value['artifacts'] = {role: value['artifacts'][role] for role in roles}
    for role, artifact in value['artifacts'].items():
        q.require(set(artifact) == {'path', 'revision', 'build'}, 'invalid diagnostic artifact fields')
        artifact.update(artifact_policy(role))
    validate(value)
    return q.plan(config_path, output, configuration=value)


def main():
    import qualify as q
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for name in ('prepare', 'run', 'report'):
        sub = commands.add_parser(name)
        sub.add_argument('input', type=Path)
        sub.add_argument('--output', type=Path, required=True)
        if name == 'run':
            sub.add_argument('--replica', type=int, default=0)
        if name == 'report':
            sub.add_argument('--accounting', type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        value = prepare(args.input, args.output)
        print('diagnostic plan sha256=' + q.routing.identity(value))
        return 0
    validate(q.read(args.input))
    if args.command == 'run':
        return 0 if q.run(args.input, args.output, replica=args.replica)['status'] == 'complete' else 1
    value = q.report(args.input, args.output, args.accounting)
    return 0 if value['status'] == 'diagnostic_complete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
