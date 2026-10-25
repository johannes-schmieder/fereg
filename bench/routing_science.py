"""Independent numerical checks and narrow Auto comparison scopes.

These checks are carried forward from the former routing producer. They do not
read campaign histories, launch processes, or construct evidence wrappers.
"""
from __future__ import annotations

import math
import numpy as np

import cmgschur
import routing
import routing_oracle
from replay_plugins import (allowed_difference, differences, exact_view,
                            numerical_scope_indices, scientific_errors, work_scope_indices)


def replay_scope(raw, arm='auto', meta=None):
    if arm != 'auto':
        raise ValueError('qualification scope applies to Auto only')
    view = exact_view(raw, False)
    numerical, work = numerical_scope_indices(False), work_scope_indices(False)
    scope = []
    for name, table in view['matrices'].items():
        indices = (range(len(table['bits'])) if name in {'RB', 'RV'} else
                   numerical.get(name, set()) | work.get(name, set()))
        scope.extend(f'/matrices/{name}/bits/{i}' for i in sorted(indices))
    scope.extend(f'/runtime_signature/{i}' for i in sorted(work.get('runtime_signature', set())))
    return scope


def cmg_polish_limits(raw, label='CMG'):
    runtime = raw['matrices']['RRUNTIME']
    status = raw['matrices']['RSTATUS']
    if (runtime['rows'], runtime['columns'], len(runtime['bits'])) != (30, 2, 60):
        raise ValueError(f'explicit {label} execution metadata shape changed')
    values = cmgschur.numbers(runtime)
    mode, poolsize = values[3], values[5]
    capacity = status['rows']
    if (values[2] != 1 or values[4] != 1 or mode not in {1, 2}
            or not math.isfinite(poolsize) or poolsize < 1 or not poolsize.is_integer()
            or type(capacity) is not int or capacity < 1 or status['columns'] != 4
            or len(status['bits']) != 4 * capacity):
        raise ValueError(f'explicit {label} execution metadata is invalid')
    # Model construction emits at most one generated column per coefficient;
    # publication bounds coefficients by RSTATUS row capacity, including omitted
    # slots. Low projects the outcome and ceil(generated_columns / poolsize)
    # regressor blocks. This capacity bound is conservative, never retained rank.
    primary = 1 if mode == 1 else 1 + (capacity + int(poolsize) - 1) // int(poolsize)
    return mode, (primary, 0 if mode == 1 else 1)


def portfolio_direct_scope(reference, raw, meta):
    """The certified zero-iteration MAP return owns only its changed family ID."""
    scope = replay_scope(raw, 'auto', meta)
    tables = [value['matrices']['RSOLVER'] for value in [reference, raw]]
    if any((table['rows'], table['columns'], len(table['bits'])) != (2, 16, 32)
           for table in tables):
        raise ValueError('portfolio direct solver shape changed')
    decoded = [cmgschur.numbers(table) for table in tables]
    for index in range(2):
        before, after = [values[16 * index:16 * (index + 1)] for values in decoded]
        if before[1] == after[1]:
            continue
        for value, row, candidate in [(reference, before, False), (raw, after, True)]:
            cmg_polish_limits(value, label='portfolio direct Auto')
            runtime = cmgschur.numbers(value['matrices']['RRUNTIME'])
            tolerance, cap = runtime[45], runtime[47]
            fields = [0, 1, 3, 10, 11, 12, 13, 14, 15]
            if (type(value.get('return_code')) is not int or value['return_code'] != 0
                    or not all(math.isfinite(v) and v >= 0 for v in row)
                    or [row[i] for i in fields] != [1, 2 if candidate else 4, 2, 0, 0, 0, 0, 0, 0]
                    or not row[2].is_integer() or (candidate and row[2] != 0)
                    or runtime[44] != 1 or runtime[46] != 1
                    or not math.isfinite(tolerance) or tolerance <= 0
                    or not math.isfinite(cap) or cap < 1 or not cap.is_integer() or row[2] > cap):
                raise ValueError('portfolio direct Auto route/counter tuple is invalid')
            # try_direct_route certifies at MapConfig::tolerance; the final
            # residual caller tightens that request to min(tolerance, 1e-14).
            # This is not the private general-LSMR certificate target.
            if candidate and row[5] > (tolerance if index == 0 else min(tolerance, 1e-14)):
                raise ValueError('portfolio direct certificate exceeds its requested target')
        scope.append(f'/matrices/RSOLVER/bits/{14 * index + 1}')
    return scope


def portfolio_scope(reference, raw, meta, arm, selected):
    """Validate Auto report tuples before admitting named route diagnostics.

    borrowed_map::{project_general_categorical,finish_general_map} and
    prepared_fit::merge_solve_report own these tuples. Repeats still compare the
    complete original fingerprint before reaching this cross-artifact check.
    """
    selected_name = next((name for name in (*routing.PORTFOLIOS, routing.GEOMETRY_MANY)
                          if routing.identity(selected) == routing.identity(routing.policy(name))), None)
    if selected_name is None or arm != 'auto':
        raise ValueError('unsupported portfolio comparison policy/request')
    # The geometry policy extends unseeded LSMR with a bounded MAP pilot and
    # accepts every exit at the requested certificate.
    geometry = selected_name == routing.GEOMETRY_MANY
    unseeded = selected_name in (routing.PORTFOLIO_UNSEEDED, routing.GEOMETRY_MANY)
    if unseeded and (not isinstance(meta, dict) or type(meta.get('categorical_intercepts')) is not bool
                     or type(meta.get('absorbed_terms')) is not int or meta['absorbed_terms'] < 0):
        raise ValueError('unseeded portfolio requires inspected fixed-effect metadata')
    if meta.get('categorical_intercepts') is True and type(meta.get('absorbed_terms')) is int and meta['absorbed_terms'] == 2:
        return portfolio_direct_scope(reference, raw, meta)
    if (meta.get('categorical_intercepts') is not True
            or type(meta.get('absorbed_terms')) is not int or meta['absorbed_terms'] < 3):
        return replay_scope(raw, arm, meta)
    terms, rows = meta['absorbed_terms'], meta.get('rows')
    if type(rows) is not int or rows < 1:
        raise ValueError('portfolio comparison requires inspected row count')
    for value, candidate in [(reference, False), (raw, True)]:
        table = value['matrices']['RSOLVER']
        if (type(value.get('return_code')) is not int or value['return_code'] != 0
                or type(table['rows']) is not int or type(table['columns']) is not int
                or (table['rows'], table['columns'], len(table['bits'])) != (2, 16, 32)):
            raise ValueError('portfolio result/solver shape changed')
        _, actions = cmg_polish_limits(value, label='portfolio Auto')
        runtime = cmgschur.numbers(value['matrices']['RRUNTIME'])
        tolerance, cap = runtime[45], runtime[47]
        if (runtime[44] != 1 or runtime[46] != 1 or not math.isfinite(tolerance)
                or tolerance <= 0 or not math.isfinite(cap) or cap < 1 or not cap.is_integer()):
            raise ValueError('portfolio runtime tolerance/iteration policy is invalid')
        decoded = cmgschur.numbers(table)
        for index in range(2):
            row = decoded[index * 16:(index + 1) * 16]
            requested, used, iterations, stop = row[:4]
            fallback, warnings, requested_pc, used_pc, pilot, contraction = row[10:]
            if (not all(math.isfinite(v) and v >= 0 for v in row)
                    or any(not row[i].is_integer() for i in [0, 1, 2, 3, 10, 11, 12, 13, 14])
                    or requested != 1 or used not in {2, 3} or stop != 2
                    or fallback not in {0, 1} or not 0 <= pilot <= min(4, iterations)
                    or iterations > cap):
                raise ValueError('portfolio Auto route/counter tuple is invalid')
            if candidate and unseeded and not geometry and pilot != 0:
                raise ValueError('unseeded portfolio cannot contain a MAP pilot')
            if candidate and geometry and pilot:
                # three_fe_map::Attempt::record marks uncertified pilot sweeps
                # as fallback work, so the bit cannot identify a continuation
                # fallback. A certified pilot returns its MAP report unmarked;
                # a declined LSMR workspace leaves only the pilot's mark.
                if fallback != 1 and (used == 3 or requested_pc != 0):
                    raise ValueError('geometry pilot fallback bit is invalid')
                fallback = int(used == 2 and requested_pc != 0 or used_pc == 2)
            # A no-LSMR MAP route has no preconditioner. A genuine LSMR
            # continuation keeps its request/resolved metadata, even if MAP
            # ultimately certifies. Low merges max work, OR fallback and sums
            # warnings; an LSMR chunk outranks a successful MAP prefix.
            absent = requested_pc == used_pc == 0
            # routing.prepare pins preconditioner=auto for both artifacts.
            pair = requested_pc == 1 and used_pc in {2, 3, 4}
            if absent:
                # LSMR workspace non-admission can reach original-input MAP
                # without an attempted report, hence without a fallback bit.
                valid = used == 2 and warnings == 0 and (not candidate or fallback == 0)
            else:
                valid = pair and (used == 3 or fallback == 1)
                if candidate and fallback and used == 3:
                    valid &= requested_pc == 1 and used_pc == 2 and warnings >= 1
                if used == 2:
                    valid &= warnings >= 1
                if not fallback and used_pc != 4:
                    valid &= warnings == 0
            # within emits at most one build warning per nonempty categorical
            # pair component, bounded by rows * choose(terms,2), per action.
            # Add one preconditioner/MAP fallback warning. Low's primary action
            # count is bounded by outcome + coefficient-capacity chunks; final
            # residual projection is one action in either execution mode.
            count = actions[0] if index == 0 else 1
            warning_cap = count * (rows * terms * (terms - 1) // 2 + 1)
            if not valid or warnings > warning_cap or (candidate and contraction != 0):
                raise ValueError('portfolio Auto preconditioner/fallback/warning tuple is invalid')
            if candidate:
                target = tolerance if index == 0 else min(tolerance, 1e-14)
                strict = target if geometry or target <= 1e-14 else max(target * 1e-2, 1e-14)
                if row[5] > strict:
                    raise ValueError('portfolio original-design certificate exceeds '
                                     + ('its requested target' if geometry else 'private Auto target'))
    scope = replay_scope(raw, arm, meta)
    # Raw columns 1,10,11,12,13,14 after removal of setup/solve time.
    return scope + [f'/matrices/RSOLVER/bits/{14 * row + column}'
                    for row in range(2) for column in [1, 8, 9, 10, 11, 12]]


def compatibility(reference, raw, reference_columns, columns, meta, arm, *, selected_policy=None):
    cmgschur.scientific_comparison(reference, raw, reference_columns, columns, meta)
    changed = differences(exact_view(reference, False), exact_view(raw, False))
    scope = (replay_scope(raw, arm, meta) if selected_policy is None else
             portfolio_scope(reference, raw, meta, arm, selected_policy))
    outside = [path for path in changed if not allowed_difference(path, scope)]
    if outside:
        raise ValueError('scoped replay changed an unaffected field: ' + '; '.join(outside[:5]))


def model_discrete_sha256(raw):
    """Exactly the model decisions compared by cmgschur.scientific_comparison."""
    cmgschur.validate_result(raw)
    tables = raw['matrices']
    names = ('RSTATUS', 'RFE', 'RCLUSTER', 'RSUBSET', 'RMARKB', 'RMARKV', 'RREC')
    if not set(names) <= set(tables) or (tables['RDOF']['rows'], tables['RDOF']['columns']) != (20, 2):
        raise ValueError('unsupported model-discrete matrix layout')
    dof = cmgschur.numbers(tables['RDOF'])
    discrete = [dof[2*row+1] for row in range(20) if row not in {4, 11}]
    return routing.identity({'matrices': {name: tables[name] for name in names},
        'presence': {name: tables[name]['bits'][::2] for name in ('RFIT', 'RDOF', 'RCERT')},
        'dof': [0.0 if value == 0 else value for value in discrete]})


def check_oracle(raw, columns, meta):
    if 'routing_oracle' in meta:
        routing_oracle.check_reference(meta['routing_oracle'], raw, columns, meta)
    if 'oracle' in meta:
        cmgschur.oracle_check(raw, columns, meta)
    if 'replay_reference' in meta:
        if meta['rows'] * meta['columns'] > 1_000_000:
            raise ValueError('imported replay oracle exceeds bounded in-memory check; provide scoped external evidence')
        data = np.memmap(columns, dtype=np.uint64, mode='r', shape=(meta['columns'], meta['rows']))
        try:
            result = {**raw, 'columns': [[f'{int(value):016x}' for value in column] for column in data]}
            errors = scientific_errors(result, meta['replay_reference'])
            if errors:
                raise ValueError('independent replay oracle: ' + '; '.join(errors[:5]))
        finally:
            del data


# Stored hierarchy metadata appended to an installed bounded-CMG route record.
ROUTE_HIERARCHY_FIELDS = ('terminal', 'levels', 'hierarchy_vertices', 'hierarchy_nonzeros', 'repeats',
                          'terminal_factor', 'terminal_factor_dimension', 'terminal_factor_nonzeros',
                          'application', 'parallel_operators', 'batch_columns')
TERMINAL_REASONS = {'Direct', 'FullContraction', 'StagnatedVertexReduction', 'StagnatedFill', 'MaximumLevels'}


def route_hierarchy(fields):
    """Validate one auto_schur_route record. Return its installed hierarchy, or
    None for a refused/not-built attempt (or a record predating installation
    attestation). An installed record without complete metadata is an error."""
    present = [key for key in ROUTE_HIERARCHY_FIELDS if key in fields]
    installed = fields.get('installed')
    if installed is None or installed == '0':
        if present:
            raise ValueError('auto_schur_route without an installation carries hierarchy metadata')
        return None
    if installed != '1':
        raise ValueError('auto_schur_route has an invalid installation flag')
    if fields.get('refusal') != 'None':
        raise ValueError('installed auto_schur_route also reports a refusal')
    missing = [key for key in ROUTE_HIERARCHY_FIELDS if key not in fields]
    if missing:
        raise ValueError('installed auto_schur_route lacks hierarchy metadata: ' + ', '.join(missing))

    def count(key, minimum=0):
        value = fields[key]
        if not value.isdigit() or int(value) < minimum:
            raise ValueError(f'auto_schur_route {key} is not a count >= {minimum}')
        return int(value)

    def series(key):
        values = fields[key].split(';')
        if not all(value.isdigit() for value in values):
            raise ValueError(f'auto_schur_route {key} is not a count series')
        return [int(value) for value in values]

    hierarchy = {'terminal': fields['terminal'], 'levels': count('levels', 1),
                 'vertices': series('hierarchy_vertices'), 'nonzeros': series('hierarchy_nonzeros'),
                 'repeats': series('repeats'), 'terminal_factor': count('terminal_factor'),
                 'terminal_factor_dimension': count('terminal_factor_dimension'),
                 'terminal_factor_nonzeros': count('terminal_factor_nonzeros'),
                 'application': fields['application'], 'parallel_operators': count('parallel_operators'),
                 'batch_columns': count('batch_columns', 1)}
    if hierarchy['terminal'] not in TERMINAL_REASONS:
        raise ValueError('auto_schur_route has an unknown terminal reason')
    if any(len(hierarchy[key]) != hierarchy['levels'] for key in ('vertices', 'nonzeros', 'repeats')):
        raise ValueError('auto_schur_route level series do not match its level count')
    # CMG factors the terminal level exactly when its terminal reason is Direct.
    factor = hierarchy['terminal_factor']
    if factor not in (0, 1) or bool(factor) != (hierarchy['terminal'] == 'Direct'):
        raise ValueError('auto_schur_route terminal factor disagrees with its terminal reason')
    if (not factor and (hierarchy['terminal_factor_dimension'] or hierarchy['terminal_factor_nonzeros'])
            or hierarchy['terminal_factor_dimension'] > hierarchy['vertices'][-1]):
        raise ValueError('auto_schur_route terminal factor dimensions are inconsistent')
    if hierarchy['application'] not in ('scalar', 'parallel') or (
            hierarchy['application'] == 'scalar' and hierarchy['parallel_operators']):
        raise ValueError('auto_schur_route application representation is inconsistent')
    return hierarchy


def resource_diagnostics(log, requested, no_fe, witness, *, expected_preset=None,
                         route_validator=route_hierarchy):
    candidate = requested not in {None, 'baseline'}
    expected = ({'injection': routing.ABLATIONS[requested]} if requested in routing.ABLATIONS else
                routing.policy(requested) if candidate else None)
    if (requested == routing.PORTFOLIO_UNSEEDED and expected_preset is not None
            and expected_preset != expected['injection']):
        raise ValueError('unseeded portfolio requires its exact private preset')
    schur_policy = expected.get('two_fe', expected) if expected else None
    actions, route, hierarchies = [], [], []
    for line in log.splitlines():
        if not line.startswith('FEREG_SOLVER_TRACE|'):
            continue
        fields = dict(part.split('=', 1) for part in line.split('|')[1:] if '=' in part)
        if fields.get('phase') == 'auto_schur_route':
            if schur_policy and ('checkpoint' not in schur_policy
                    or fields.get('checkpoint') != str(schur_policy['checkpoint'])
                    or fields.get('samples') != str(schur_policy['trees'])):
                raise ValueError('actual Schur route does not match requested policy')
            route.append(fields)
            hierarchies.append(route_validator(fields))
        if fields.get('phase') != 'routing_action':
            continue
        if expected and fields.get('preset') != (expected_preset if expected_preset is not None else expected['injection']):
            raise ValueError('routing_action does not attest the requested preset')
        if fields.get('final') not in {'0', '1'}:
            raise ValueError('routing_action lacks its projection/final action classification')
        action = {'final': fields.get('final'), 'preset': fields.get('preset')}
        # The optional construction-work cap was retired on 2026-10-19; native
        # iteration limits and joint memory admission remain checked caps.
        for target, used, limit in [('iterations', 'iterations', 'native_limit'),
                                    ('memory_bytes', 'memory_bytes', 'memory_limit')]:
            try:
                action[target + '_used'] = int(fields[used])
                action[target + '_limit'] = int(fields[limit])
            except (KeyError, ValueError) as error:
                raise ValueError('incomplete routing_action counters') from error
            if not 0 <= action[target + '_used'] <= action[target + '_limit']:
                raise ValueError(f'{target}: action exceeded its cap')
        if fields.get('certified') != '1':
            raise ValueError('routing_action was not certified')
        actions.append(action)
    if candidate and not actions and not (no_fe and 'no_fe_exact_path' in witness):
        raise ValueError('candidate has no actual routing_action or bound exact-no-FE witness')
    checks = {'iteration_caps': 'passed', 'memory_caps': 'passed'}
    return {'action_resources': actions, 'resource_checks': checks, 'route_trace': route,
            'route_hierarchy': hierarchies,
            'resource_basis': 'actual traced prerequisite' if actions else
                              'bound exact-no-FE witness' if candidate else 'bound legacy resource witness'}
