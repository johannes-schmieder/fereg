"""Matched calibration labels; unavailable counterfactuals stay explicit.

Consumes retained complete-call records, not clocks or live native processes.
Only recorded mirrored pairs enter fitting; warmups still attest repeatability.
"""
from __future__ import annotations

from fractions import Fraction

from two_fe_policy import ACTION, ARMS, require


def action_key(action):
    return tuple(action['end'][key] for key in ACTION)


def stable_actions(actions):
    return [{
        'end': {k: v for k, v in action['end'].items() if not k.endswith('_ns')},
        'checkpoints': [{k: v for k, v in point.items() if k != 'stamp_ns'}
                        for point in action['checkpoints']],
    } for action in actions]


def validate_actions(actions, memory, workers):
    require(actions and memory in ('fast', 'low'), 'missing projection schedule')
    ends = [action['end'] for action in actions]
    first, final = ends[0], ends[-1]
    require(first['role'] == ('packed' if memory == 'fast' else 'outcome')
            and final['role'] == 'final' and final['final'] == 1 and final['r'] == 1,
            'actual projection roles differ from caller schedule')
    if memory == 'fast':
        require(len(ends) == 2 and first['r'] == first['total'] + 1,
                'packed projection has invented future actions')
    else:
        require(first['r'] == 1 and first['block'] > 0, 'invalid outcome projection')
        consumed = 0
        for end in ends[1:-1]:
            require(end['role'] == 'regressor' and end['start'] == consumed
                    and end['total'] == first['total'] and end['block'] == first['block']
                    and end['end'] == min(consumed + first['block'], first['total'])
                    and end['r'] == end['end'] - end['start'], 'invalid regressor projection range')
            consumed = end['end']
        require(consumed == first['total'], 'missing scheduled regressor columns')
    require(all(end['final'] == 0 and end['tolerance_bits'] == first['tolerance_bits']
                for end in ends[:-1]), 'primary projection accuracy changed')
    require(all(end['stride'] == first['stride'] for end in ends), 'retained bank stride changed')
    attempts = set()
    history = None
    accuracy = None
    for action in actions:
        end = action['end']
        key = (end['tolerance_bits'], end['final'])
        if key != accuracy:
            history = None
            accuracy = key
        for point in action['checkpoints']:
            require(point['workers'] == workers, 'actual admitted workers differ from frozen cell')
            require(point['remaining'] > 0 and point['state'] is not None, 'invalid matching checkpoint')
            expected = ([None] * 5 if history is None else
                        [history[k] for k in ('width', 'actions', 'steps', 'live', 'original')])
            require([point[k] for k in ('history_width', 'history_actions', 'history_steps',
                                       'history_live', 'history_original')] == expected,
                    'history includes current, failed or incompatible work')
            if point['build']:
                require(key not in attempts, 'accuracy key consumes more than one opportunity')
                attempts.add(key)
        if end['role'] == 'regressor':
            if history is None or history['width'] != end['r']:
                history = dict(width=end['r'], actions=0, steps=0, live=0, original=0)
            if end['steps'] is None or end['live'] is None:
                history = None
            else:
                history['actions'] += 1
                history['steps'] += end['steps']
                history['live'] += end['live']
                history['original'] += end['r']


def validate_outcomes(call):
    """Bind economic grants to the ordered native outcomes and action reports.

    Direct/initial-certificate returns legitimately have no Schur grant. A
    retained installation also needs no new outcome on later actions. Route
    records expose total iterations; correction labels expose local age only,
    so their unknown preceding-epoch offset is bounded, never invented.
    """
    import routing
    from routing_science import route_hierarchy

    role = call['execution']
    require(role in ARMS, 'unknown calibration outcome control')
    expected = routing.policy(ARMS[role])
    actions, resources = call['policy_actions'], call['action_resources']
    routes, hierarchies = call['route_trace'], call['route_hierarchy']
    require(len(actions) == len(resources), 'economic/action outcome count changed')
    require(len(routes) == len(hierarchies), 'route/hierarchy outcome count changed')
    cursor, installed = 0, False
    for action, resource in zip(actions, resources):
        end = action['end']
        require(resource['final'] == str(end['final'])
                and resource['preset'] == expected['injection'],
                'economic action differs from native action classification')
        for used, limit in (('iterations_used', 'iterations_limit'),
                            ('memory_bytes_used', 'memory_bytes_limit')):
            require(type(resource[used]) is int and type(resource[limit]) is int
                    and 0 <= resource[used] <= resource[limit],
                    'native action exceeds its resource bound')
        grants = [point for point in action['checkpoints'] if point['build']]
        require(len(grants) <= 1, 'multiple economic grants in one action')
        if not grants:
            require(end['installed'] == 0 and all(end[key] is None for key in
                    ('build_ns', 'restart_ns', 'startup_ns', 'continuation_ns')),
                    'ungranted action carries constructor intervals')
            continue
        require(not installed and cursor < len(routes),
                'economic grant lacks a new native constructor outcome')
        point, route = grants[0], routes[cursor]
        hierarchy = hierarchies[cursor]
        cursor += 1
        require(role != 'never' and point['local'] == expected['two_fe']['checkpoint']
                and route.get('checkpoint') == str(expected['two_fe']['checkpoint'])
                and route.get('samples') == str(expected['two_fe']['trees']),
                'native constructor differs from forced checkpoint')
        encoded_iterations = route.get('iterations', '')
        require(isinstance(encoded_iterations, str) and encoded_iterations.isascii()
                and encoded_iterations.isdigit(), 'invalid constructor iteration count')
        iterations = int(encoded_iterations)
        require(iterations <= resource['iterations_used']
                and (iterations == point['local'] if point['epoch'] == 0
                     else point['epoch'] == 1 and iterations > point['local']),
                'constructor checkpoint lies outside its native action')
        require(route.get('installed') in ('0', '1')
                and int(route['installed']) == end['installed']
                and end['build_ns'] is not None,
                'economic installation differs from native outcome')
        require(route_hierarchy(route) == hierarchy,
                'retained hierarchy differs from native constructor')
        if end['installed']:
            require(route.get('refusal') == 'None' and hierarchy is not None,
                    'installed constructor lacks its hierarchy')
            installed = True
        else:
            require(route.get('refusal') == 'Some(MemoryBudget)' and hierarchy is None,
                    'constructor refusal is not the permitted memory refusal')
    require(cursor == len(routes), 'native constructor outcome has no economic grant')


def paired_cell(cell, *, platform, seed, family, memory, workers):
    """Return usable paired observations plus every unavailable intervention.

    An absent/mismatched state is retained as an unavailable label; malformed
    schedules, changed repeats or opportunity violations fail the harness.
    """
    warmups = {call['execution']: call for call in cell['warmups']}
    require(set(warmups) == set(ARMS) and len(cell['warmups']) == 3,
            'missing/duplicate calibration warmup')
    require(len(cell['blocks']) == 1 and cell['blocks'][0]['status'] == 'complete',
            'calibration needs one complete mirrored block')
    calls = {role: [] for role in ARMS}
    for call in cell['blocks'][0]['calls']:
        require(call['execution'] in calls, 'unknown calibration control')
        calls[call['execution']].append(call)
    require(all(len(group) == 2 for group in calls.values()), 'missing/extra recorded calibration call')
    for role, group in calls.items():
        reference = warmups[role]
        for call in [reference, *group]:
            validate_actions(call['policy_actions'], memory, workers)
            validate_outcomes(call)
            require(call['fingerprint'] == reference['fingerprint'], 'calibration science repeat changed')
            require(stable_actions(call['policy_actions']) == stable_actions(reference['policy_actions']),
                    'non-timing policy labels changed within a fixed-worker repeat')
            for key in ('route_trace', 'route_hierarchy', 'action_resources'):
                require(call[key] == reference[key], 'calibration route/resource repeat changed')
    context = dict(platform=platform, seed=seed, family=family, memory=memory, workers=workers)
    observations, unavailable = [], []
    for role in ('force16', 'force32'):
        forced = [call['policy_actions'] for call in calls[role]]
        never = [call['policy_actions'] for call in calls['never']]
        require(all([action_key(a) for a in f] == [action_key(a) for a in n]
                    for f, n in zip(forced, never)), 'control changed immutable action schedule')
        interventions = [index for index, action in enumerate(forced[0])
                         if any(point['build'] for point in action['checkpoints'])]
        if not interventions:
            unavailable.append(dict(context, control=role, reason='no_intervention'))
        for index in interventions:
            paired, reason = [], None
            for f, n in zip(forced, never):
                point = next(point for point in f[index]['checkpoints'] if point['build'])
                match = next((p for p in n[index]['checkpoints']
                              if (p['epoch'], p['local']) == (point['epoch'], point['local'])), None)
                if not f[index]['end']['installed']:
                    reason = 'constructor_refused'
                elif match is None:
                    reason = 'missing_incumbent_checkpoint'
                elif {k: v for k, v in point.items() if k not in ('build', 'stamp_ns')} != {
                        k: v for k, v in match.items() if k not in ('build', 'stamp_ns')}:
                    reason = 'pre_intervention_state_mismatch'
                elif any(point[k] is None for k in ('forecast_h', 'forecast_d', 'classes')):
                    reason = 'unknown_forecast'
                if reason:
                    break
                measured = {name: f[index]['end'][field] for name, field in (
                    ('B', 'build_ns'), ('R', 'restart_ns'), ('startup', 'startup_ns'), ('C', 'continuation_ns'))}
                measured['I'] = n[index]['end']['elapsed_ns'] - match['stamp_ns']
                measured['observed_suffix_gain'] = (sum(a['end']['elapsed_ns'] for a in n[index+1:])
                                                   - sum(a['end']['elapsed_ns'] for a in f[index+1:]))
                paired.append(measured)
            identity = dict(context, control=role, action=index, checkpoint=point)
            if reason:
                unavailable.append(dict(identity, reason=reason))
                continue
            observations.append(dict(identity, labels={k: Fraction(paired[0][k] + paired[1][k], 2)
                                                       for k in paired[0]}))
    return observations, unavailable
