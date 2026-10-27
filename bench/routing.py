"""Request framing and fixed routing policies shared by replay and qualification.

Historical tournament orchestration lives in Git; this module does not select
candidates, reconstruct previous campaigns, or adjudicate performance.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import shutil

from native_io import case_header

ABLATIONS = {name: 'routing_' + name for name in ['diagonal', 'cells', 'direct']}
PREFIXES = {'prefix-none': ('none', 0), 'prefix-map4': ('map4', 4), 'prefix-it4': ('it4', 8)}
PORTFOLIO_V1 = 'portfolio-v1'
PORTFOLIO_UNSEEDED = 'portfolio-unseeded'
PORTFOLIOS = (PORTFOLIO_V1, PORTFOLIO_UNSEEDED)
INCREMENTAL = ('incremental-many', 'incremental-two')
REMAINING = ('two-remaining-v1', 'two-remaining-never',
             'two-remaining-force16', 'two-remaining-force32')
GATES = 'two-gates-v1'
GATES_V2 = 'two-gates-v2'
GATE_RULES = {GATES: 'v1', GATES_V2: 'stall-v2'}
GEOMETRY_MANY = 'many-geometry-v1'
COMPACT_IT = 'it-coefficients'


def incremental_eligible(cell, family):
    family = family.removeprefix('incremental-')
    if family not in {'many', 'two'}:
        raise ValueError('unknown incremental family')
    count = cell.get('absorbed_terms')
    return (cell.get('categorical_intercepts') is True and type(count) is int
            and (count >= 3 if family == 'many' else count == 2))


def identity(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     allow_nan=False).encode()).hexdigest()


def seal(value):
    return {'sha256': identity(value), 'value': value}


def unseal(envelope):
    value = envelope['value']
    if envelope.get('sha256') != identity(value):
        raise ValueError('sealed input was changed')
    return value


def sha(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def policy(arm):
    if arm == 'baseline':
        return None
    if arm == GATES:
        base = policy(PORTFOLIO_UNSEEDED)
        return {**base, 'family': 'two', 'injection': 'routing_two_gates_v1',
                'decision': 'direct-loss-gates-v1', 'first_decision': 16,
                'selected_features': ['final'], 'progress_samples': 0,
                'two_fe': {**base['two_fe'], 'checkpoint': 0}}
    if arm == GATES_V2:
        return {**policy(GATES), 'injection': 'routing_two_gates_v2',
                'decision': 'stall-gates-v2', 'first_decision': 4,
                'stall': {'ages': [4, 8, 12], 'terminal': 16, 'statistic': 'energy_since_pilot_start',
                          'threshold': 0.5}}
    if arm in REMAINING:
        base = policy(PORTFOLIO_UNSEEDED)
        checkpoint = {'two-remaining-v1': 0, 'two-remaining-never': 2**64 - 1,
                      'two-remaining-force16': 16, 'two-remaining-force32': 32}[arm]
        return {**base, 'family': 'two', 'injection': 'routing_' + arm.replace('-', '_'),
                'decision': 'remaining-cost-v1', 'observation_cadence': 8,
                'first_decision': 16, 'calibration_control': arm != REMAINING[0],
                'two_fe': {**base['two_fe'], 'checkpoint': checkpoint}}
    if arm == GEOMETRY_MANY:
        return {**policy(PORTFOLIO_UNSEEDED), 'family': 'many', 'injection': 'routing_many_geometry_v1',
                'certificate': 'requested', 'map_pilot': 'geometry_checkpoint_v1',
                'recurrence': {'margin': 10, 'floor': 1e-14,
                               'certificate_miss': 'one_seeded_correction_at_hundredth'},
                'preconditioner_trial': {'first': 'diagonal', 'escalation': 'additive',
                                         'decision': 'projected_iterations_v1',
                                         'iteration_budget': 40}}
    if arm in INCREMENTAL:
        return {**policy(PORTFOLIO_UNSEEDED), 'family': arm.removeprefix('incremental-'),
                'injection': 'routing_' + arm.replace('-', '_')}
    if arm == PORTFOLIO_V1:
        return {'prefix': 'map4', 'steps': 4, 'map_sweeps': 4, 'prefix_min_terms': 3,
                'replace_existing_prefix': True, 'optional_cmg_handoff': False,
                'certificate': 'private_auto', 'injection': 'routing_portfolio_v1',
                'two_fe': {'checkpoint': 16, 'trees': 1, 'degree': 5, 'seed': 0,
                           'construction': 'synchronous', 'canonical_cache': True,
                           'small_direct': {'selection': 'dimension_upper_bound_v1',
                                            'preflight_work': 1024,
                                            'structural_iteration_allowance': 4,
                                            'choice': 'direct_or_incumbent_cmg'}}}
    if arm == PORTFOLIO_UNSEEDED:
        return {**policy(PORTFOLIO_V1), 'prefix': 'none', 'steps': 0, 'map_sweeps': 0,
                'injection': 'routing_portfolio_unseeded'}
    if arm in PREFIXES:
        name, sweeps = PREFIXES[arm]
        return {'prefix': name, 'steps': 0 if name == 'none' else 4, 'map_sweeps': sweeps,
                'replace_existing_prefix': True, 'optional_cmg_handoff': False,
                'injection': 'routing_prefix_' + name}
    overlap = arm.startswith('overlap-')
    match = re.fullmatch(r'h(4|8|16|32)-k([012])', arm.removeprefix('overlap-'))
    if match is None:
        raise ValueError('routing policy must be baseline or h{4,8,16,32}-k{0,1,2}')
    h, k = map(int, match.groups())
    if overlap and h == 4:
        raise ValueError('overlap requires a checkpoint after four iterations')
    result = {'checkpoint': h, 'trees': k, 'degree': 5, 'seed': 0,
              'injection': f'routing_{"overlap_" if overlap else ""}h{h}_k{k}'}
    if overlap:
        result['overlap'] = True
    return result


def prepare(source, destination, arm, threads):
    """Preserve framed input/model bytes while selecting a sanctioned route arm."""
    selected = ({'injection': 'it_coefficients'} if arm == COMPACT_IT else
                {'injection': ABLATIONS[arm]} if arm in ABLATIONS else policy(arm))
    if type(threads) is not int or not 1 <= threads <= 1024:
        raise ValueError('threads must be 1..1024')
    with source.open('rb') as stream:
        shape, arguments = case_header(stream)
        if arguments[0] != b'estimate-ols':
            raise ValueError('routing policy requests require OLS')
        if any(a.startswith(b'inject=') or a.startswith(b'break_marker_token=') for a in arguments):
            raise ValueError('source already contains developer injection')
        updates = {'solver': 'it' if arm == COMPACT_IT else 'auto',
                   'preconditioner': 'off' if arm == COMPACT_IT else 'auto', 'threads': str(threads)}
        if selected:
            updates['inject'] = selected['injection']
        removed = {*updates, 'cmgdegree', 'cmgsamples', 'cmgseed'}
        arguments = [a for a in arguments if a.split(b'=', 1)[0].decode() not in removed]
        arguments += [f'{key}={value}'.encode() for key, value in updates.items()]
        shape[2] = len(arguments)
        with destination.open('xb') as out:
            out.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, shape)) + '\n').encode())
            for arg in arguments:
                out.write(str(len(arg)).encode() + b'\n' + arg + b'\n')
            shutil.copyfileobj(stream, out, 1 << 20)
    return shape


def read_json(path):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            if key in out:
                raise ValueError(f'duplicate JSON key {key}')
            out[key] = value
        return out
    return json.loads(path.read_text(), object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(f'nonfinite JSON {value}')))
