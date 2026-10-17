#!/usr/bin/env python3
"""Freeze and select a bounded routing tournament; this never runs benchmarks.

Records must come from the maintained native timing/science harness. Failed
candidates are excluded. Optional observed unavailability leaves runtime
unidentified; missing or invalid reference evidence blocks the panel. A and B select;
C only accepts or rejects the policy frozen by B. Bootstrap coverage is nominal
per comparison, not simultaneous over this tournament.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import statistics

from cmgschur_cases import scratch
from native_io import case_header
from time_exact import compare

CHECKPOINTS = (4, 8, 16, 32)
ABLATIONS = {name: 'routing_' + name for name in
             ['diagonal', 'cells', 'cells_v2', 'cells_v2_df', 'cells_deferred', 'cells_m', 'cells_m_lanes', 'cells_s', 'cells_s2', 'cells_e', 'fit_owned', 'cells_e_final', 'direct']}
PREFIXES = {'prefix-none': ('none', 0), 'prefix-map4': ('map4', 4), 'prefix-it4': ('it4', 8)}
PORTFOLIO_V1 = 'portfolio-v1'
PORTFOLIO_UNSEEDED = 'portfolio-unseeded'
PORTFOLIOS = (PORTFOLIO_V1, PORTFOLIO_UNSEEDED)
COMPACT_IT = 'it-coefficients'
EXPERIMENT_KINDS = {'routing', 'prefix', 'overlap', 'portfolio'}
PREFIX_COVERAGE = {'small-third', 'near-confounded-third', 'four-fe', 'eight-fe', 'sixteen-fe'}
TREES = (0, 1, 2)
REPETITIONS = {'A': 3, 'B': 7, 'C': 7}
FAILURES = {'unavailable', 'blocked', 'not_run'}
ITERATION_ERRORS = {
    'borrowed Irons-Tuck exhausted its iteration budget without certification': {'it', 'auto'},
    'reusable MAP exhausted its iteration budget without certification': {'map', 'auto'},
    'reusable categorical MAP exhausted its iteration budget without certification': {'map', 'auto'},
    'two-FE categorical MAP PCG exhausted its iteration budget without certification': {'auto'},
    'borrowed MAP exhausted its iteration budget without a passing final certificate': {'map', 'auto'},
}
CMGSCHUR_NONCONVERGENCE = 'cmgschur did not certify within its iteration budget'
FAILURE_POLICY = {
    'version': 'availability-v2',
    'reliability_gate': 'all planned calls succeed; no partial-repetition timing',
    'terminal_failures': ['iteration_exhausted', 'certification_nonconvergence', 'timeout'],
    'remaining_calls': 'not_run with the original terminal failure bound by hash',
    'optional_invalid_science_or_unresolved': 'blocks the panel',
    'required_reference_failure': 'blocks the panel',
    'candidate_failure': 'excludes that candidate',
    'per_call_timeout_s': 120.0,
    'aggregate_budget_s': 14400.0,
}


def arm_role(manifest, arm):
    if arm in manifest['protections']:
        return 'required_reference'
    return 'candidate' if arm in manifest['candidates'] else 'optional_comparator'


def terminal_failure(row, manifest):
    """Only a dispatched, bounded, established terminal failure is availability."""
    failure = row.get('failure', {})
    if row.get('status') != 'unavailable' or failure.get('dispatched') is not True:
        return False
    if not sha(failure.get('log_sha256')) or not sha(failure.get('prepared_sha256')):
        return False
    for action in failure.get('action_resources', []):
        for resource in ['iterations', 'setup_work', 'memory_bytes']:
            used, limit = action.get(resource + '_used'), action.get(resource + '_limit')
            if type(used) is not int or type(limit) is not int or not 0 <= used <= limit:
                return False
    if failure.get('prepared_sha256') != row.get('prepared_sha256'):
        return False
    if failure.get('kind') == 'iteration_exhausted':
        solver = manifest['arms'][row['arm']].get('solver', row['arm'])
        if arm_role(manifest, row['arm']) != 'optional_comparator' or solver in ABLATIONS:
            solver = 'auto'
        return (failure.get('process_completed') is True
                and failure.get('return_code') == 459
                and solver in ITERATION_ERRORS.get(failure.get('native_error'), set())
                and sha(failure.get('result_sha256')))
    if failure.get('kind') == 'certification_nonconvergence':
        return (arm_role(manifest, row['arm']) == 'optional_comparator'
                and manifest['arms'][row['arm']].get('solver', row['arm']) == 'cmgschur'
                and failure.get('process_completed') is True and failure.get('return_code') == 430
                and failure.get('native_error_label') == 'nonconvergence'
                and failure.get('native_error') == CMGSCHUR_NONCONVERGENCE
                and sha(failure.get('result_sha256')))
    if failure.get('kind') == 'timeout':
        limit = manifest['failure_policy']['per_call_timeout_s']
        return (failure.get('call_limit_s') == limit
                and type(failure.get('elapsed_s')) in (int, float)
                and math.isfinite(failure['elapsed_s']) and failure['elapsed_s'] >= limit)
    return False


def observation_identity(row):
    return identity({k: v for k, v in row.items() if k != 'verified'})


def verified_record(row, manifest):
    checks = row.get('resource_checks', {})
    setup_ok = checks.get('setup_caps') == 'passed' or (checks.get('setup_caps') == 'not_applicable'
                and (row['arm'] not in manifest['candidates'] or row['arm'] == 'baseline'))
    verified = (row.get('certificate_pass') is True and row.get('science_pass') is True
                and sha(row.get('science_sha256')) and sha(row.get('witness_sha256'))
                and checks.get('iteration_caps') == 'passed'
                and checks.get('memory_caps') == 'passed' and setup_ok)
    for action in row.get('action_resources', []):
        for resource in ['iterations', 'setup_work', 'memory_bytes']:
            used, limit = action.get(resource + '_used'), action.get(resource + '_limit')
            if type(used) is not int or type(limit) is not int or not 0 <= used <= limit:
                verified = False
    return verified


def arm_outcome(manifest, rows, prerequisite=None):
    """All rows remain accountable; a partial timing sample never yields a mean."""
    if any(row is None for row in rows):
        return {'status': 'blocked', 'reason': 'missing observations'}
    if prerequisite and prerequisite.get('status') == 'passed':
        if not verified_record(prerequisite, manifest):
            return {'status': 'blocked', 'reason': 'unverified scientific prerequisite'}
        rows = [{**prerequisite, 'verified': True}, *rows]
    failures = ([prerequisite] if prerequisite and prerequisite.get('status') != 'passed' else [])
    failures += [row for row in rows if row.get('status') in {'unavailable', 'blocked'}]
    stopped = bool(prerequisite and prerequisite.get('status') != 'passed')
    for row in rows:
        if stopped and row.get('status') != 'not_run':
            return {'status': 'blocked', 'reason': 'call after failed arm; no retries permitted'}
        if row.get('status') in {'unavailable', 'blocked'}:
            stopped = True
    for row in rows:
        if row.get('status') == 'not_run':
            if not any(row.get('blocked_by_sha256') == observation_identity(failure) for failure in failures):
                return {'status': 'blocked', 'reason': 'unbound skipped observation'}
        elif row.get('status') == 'passed' and not row.get('verified', False):
            return {'status': 'blocked', 'reason': 'uncertified science or resource checks'}
        elif row.get('status') not in {'passed', 'unavailable', 'blocked'}:
            return {'status': 'blocked', 'reason': 'unknown observation status'}
    passed = [row for row in rows if row['status'] == 'passed']
    if len({row['science_sha256'] for row in passed}) > 1:
        return {'status': 'blocked', 'reason': 'nonexact repeat'}
    if len({row['witness_sha256'] for row in passed}) > 1:
        return {'status': 'blocked', 'reason': 'scientific witness changed'}
    if failures:
        if all(terminal_failure(row, manifest) for row in failures):
            return {'status': 'unavailable', 'reason': 'observed terminal failure; all-success reliability gate failed',
                    'failure_sha256': [observation_identity(row) for row in failures],
                    'successful_calls': len(passed), 'runtime_identified': False}
        return {'status': 'blocked', 'reason': 'invalid science, resource or unresolved native failure'}
    if not passed or len(passed) != len(rows):
        return {'status': 'blocked', 'reason': 'incomplete observations'}
    return {'status': 'passed', 'successful_calls': len(passed), 'runtime_identified': True}

COVERAGE = {'no-fe', 'one-fe', 'dense', 'weak', 'small-retained', 'small-third',
            'four-fe', 'eight-fe', 'weighted', 'repeated-cells', 'wide-rhs', 'slopes'}
SAMPLED_COVERAGE = {'high-degree-mixed', 'high-degree-weak', 'high-degree-stress'}
PORTFOLIO_COVERAGE = COVERAGE | SAMPLED_COVERAGE | {
    'sixteen-fe', 'disconnected', 'near-confounded-third', 'weight-dynamic',
    'inactive-columns', 'low-saved'}


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


def policy_name(value):
    if value is None:
        return 'baseline'
    choices = [*PORTFOLIOS, *PREFIXES, *(f'h{h}-k{k}' for h in CHECKPOINTS for k in TREES),
               *(f'overlap-h{h}-k{k}' for h in CHECKPOINTS[1:] for k in TREES)]
    for name in choices:
        if (identity(policy(name)) == identity(value) if name in PORTFOLIOS else policy(name) == value):
            return name
    raise ValueError('unknown or incomplete fixed routing policy')


def portfolio_candidate(manifest):
    """Return one sanctioned portfolio challenger, without recursive validation."""
    candidates = manifest.get('candidates')
    if (manifest.get('experiment_kind') != 'portfolio' or not isinstance(candidates, list)
            or len(candidates) != 2 or any(not isinstance(a, str) for a in candidates)
            or 'baseline' not in candidates):
        raise ValueError('portfolio candidate set requires baseline and one sanctioned policy')
    challengers = [arm for arm in candidates if arm != 'baseline']
    if len(challengers) != 1 or challengers[0] not in PORTFOLIOS:
        raise ValueError('portfolio candidate set requires baseline and one sanctioned policy')
    return challengers[0]


def prepare(source, destination, arm, threads):
    """Preserve framed input/model bytes while selecting a sanctioned route arm."""
    selected = ({'injection': 'it_coefficients'} if arm == COMPACT_IT else
                {'injection': ABLATIONS[arm]} if arm in ABLATIONS else policy(arm))
    if type(threads) is not int or not 1 <= threads <= 1024:
        raise ValueError('threads must be 1..1024')
    with source.open('rb') as stream:
        shape, arguments = case_header(stream)
        if arguments[0] != b'estimate-ols':
            raise ValueError('initial routing tournament is OLS only')
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


def conservative(arm):
    if arm in {'serial', 'overlap'}:
        return (int(arm == 'overlap'), 0, 0, 0)
    if arm in PREFIXES:
        return (1, list(PREFIXES).index(arm), 0, 0)
    if arm in PORTFOLIOS:
        return (1, 0, 0, 0)
    p = policy(arm)
    return (0, 0, 0, 0) if p is None else (1, p['trees'] != 0, -p['checkpoint'], p['trees'])


def preceding_bodies(envelopes):
    """Read frozen metadata only; never open earlier datasets or holdout results."""
    bodies = set()
    for envelope in envelopes:
        previous = unseal(envelope)
        for cells in previous['panels'].values():
            for cell in cells:
                if not sha(cell.get('body_sha256')):
                    raise ValueError('prior manifest has an invalid body identity')
                bodies.add(cell['body_sha256'])
        bodies.update(preceding_bodies(previous.get('prior_manifests', [])))
    return bodies


def serial_basis(manifest):
    basis = manifest.get('serial_qualification', {})
    previous = unseal(basis['manifest'])
    decision = unseal(basis['decision'])
    if previous.get('experiment_kind', 'routing') != 'routing':
        raise ValueError('overlap requires a complete qualified serial routing policy')
    validate_manifest(previous)
    arm = decision.get('promoted')
    ranking = decision.get('ranking', [])
    if (decision.get('schema') != 2 or decision.get('experiment_kind', 'routing') != 'routing'
            or len(ranking) != 1 or ranking[0].get('arm') != arm
            or ranking[0].get('pass') is not True or ranking[0].get('data_valid') is not True
            or decision.get('failure_policy') != previous['failure_policy']
            or decision.get('blocking_references') != []
            or not sha(decision.get('selection_sha256'))):
        raise ValueError('overlap requires the complete passing serial C decision and evidence bindings')
    if (decision.get('stage') != 'C' or decision.get('status') != 'qualified'
            or decision.get('manifest_sha256') != identity(previous)
            or decision.get('frozen_policy') != arm or arm not in previous['candidates']
            or not sha(decision.get('records_sha256')) or arm == 'baseline'):
        raise ValueError('overlap requires the preceding sealed C qualification, never only a B winner')
    selected = policy(arm)
    if selected['checkpoint'] <= 4:
        raise ValueError('qualified serial checkpoint is ineligible for overlap')
    if manifest.get('sampled_qualification', {}) != previous.get('sampled_qualification', {}):
        raise ValueError('overlap must retain its serial sampling qualification bindings')
    if (manifest['arms']['serial']['policy'] != selected
            or manifest['arms']['overlap']['policy'] != policy('overlap-' + arm)
            or any(manifest['arms'][name]['sha256'] != previous['arms'][arm]['sha256'] for name in ['serial', 'overlap'])
            or manifest['arms']['baseline']['sha256'] != previous['arms']['baseline']['sha256']):
        raise ValueError('overlap changed the qualified complete serial policy or artifacts')
    return previous


def validate_manifest(manifest):
    ablation = manifest.get('purpose') == 'ablation_only'
    kind = manifest.get('experiment_kind', 'routing')
    if kind not in EXPERIMENT_KINDS or (ablation and kind != 'routing'):
        raise ValueError('declare one isolated experiment kind; no Cartesian combinations')
    if manifest.get('schema') != 2 or manifest.get('warmups') != 1:
        raise ValueError('require schema 2 and exactly one warmup')
    if manifest.get('failure_policy') != FAILURE_POLICY:
        raise ValueError('require the frozen availability policy and call/panel bounds')
    if manifest.get('repetitions') != REPETITIONS:
        raise ValueError('fixed A/B/C repetitions are 3/7/7')
    arms = manifest['arms']
    candidates = manifest['candidates']
    protections = manifest['protections']
    if not candidates or len(candidates) != len(set(candidates)) or ('baseline' not in candidates and kind != 'overlap'):
        raise ValueError('unique candidates must include immutable baseline')
    if protections not in ([['baseline', 'serial']] if kind == 'overlap' else
                           [['baseline', 'incumbent']] if kind == 'prefix' else
                           [['baseline'], ['baseline', 'incumbent']]):
        raise ValueError('protect baseline and optionally the stage incumbent')
    if any(a not in arms for a in [*candidates, *protections]):
        raise ValueError('candidate/protection artifact missing')
    if arms['baseline'].get('policy') is not None:
        raise ValueError('immutable baseline must remain uninjected in every experiment')
    if not sha(manifest['harness_sha256']) or any(not sha(v['sha256']) for v in arms.values()):
        raise ValueError('require frozen harness and arm artifact hashes')
    incumbent_policy = arms.get('incumbent', {}).get('policy')
    if incumbent_policy is not None:
        name = policy_name(incumbent_policy)
        if name.startswith('overlap-') or name in PORTFOLIOS or kind in {'prefix', 'portfolio'}:
            raise ValueError('stage incumbent contains an unrelated experiment policy')
    for arm, config in arms.items():
        if config.get('role') != arm_role(manifest, arm):
            raise ValueError(f'{arm}: role does not match its declared protection/candidate/comparator position')
        allowed = {'sha256', 'policy', 'role'} if arm in candidates + protections else {'sha256', 'solver', 'cmgsamples', 'role', 'injection'}
        if set(config) - allowed:
            raise ValueError(f'{arm}: unknown arm configuration fields')
        if arm == COMPACT_IT:
            if config.get('solver') != 'it' or config.get('injection') != 'it_coefficients':
                raise ValueError('compact IT requires its distinct name and exact solver/injection tuple')
        elif 'injection' in config:
            raise ValueError('arbitrary comparator injection is unsupported; public IT keeps its meaning')
        if 'cmgsamples' in config and (config.get('solver', arm) != 'cmgschur'
                or type(config['cmgsamples']) is not int or config['cmgsamples'] not in TREES):
            raise ValueError('CMG-Schur comparator samples must be 0/1/2')
    sampled = manifest.get('sampled_qualification', {})
    counts = {0}
    if not isinstance(sampled, dict) or any(k not in {'1', '2'} or not sha(v) for k, v in sampled.items()):
        raise ValueError('sampled qualification maps counts 1/2 to scientific evidence hashes')
    counts.update(map(int, sampled))
    if kind == 'portfolio' and set(sampled) != {'1'}:
        raise ValueError('portfolio requires exactly the sample-1 scientific qualification binding')
    if (ablation or kind == 'prefix') and sampled:
        raise ValueError('this isolated experiment does not declare sampled policy candidates')
    expected = ({'baseline'} if ablation else {'baseline', *PREFIXES} if kind == 'prefix' else
                {'baseline', portfolio_candidate(manifest)} if kind == 'portfolio' else
                {'serial', 'overlap'} if kind == 'overlap' else
                {'baseline', *(f'h{h}-k{k}' for h in CHECKPOINTS for k in counts)})
    if set(candidates) != expected:
        raise ValueError('freeze exactly baseline and one portfolio policy as the candidate set' if kind == 'portfolio'
                         else 'freeze the full four-checkpoint grid for each qualified tree count')
    candidate_hashes = {arms[a]['sha256'] for a in candidates if a != 'baseline'}
    if not ablation and len(candidate_hashes) != 1:
        raise ValueError('policy candidates must share one frozen artifact')
    for arm in candidates:
        expected_policy = (arms['serial']['policy'] if kind == 'overlap' and arm == 'serial' else
                           policy('overlap-' + policy_name(arms['serial']['policy'])) if kind == 'overlap' else policy(arm))
        if (identity(arms[arm].get('policy')) != identity(expected_policy) if arm in PORTFOLIOS
                else arms[arm].get('policy') != expected_policy):
            raise ValueError(f'{arm}: policy and sanctioned injection must match')
    if set(manifest['panels']) != ({'A'} if ablation else set(REPETITIONS)):
        raise ValueError('predeclare all three panels')
    prior = manifest.get('prior_manifests', [])
    if kind != 'routing' and not prior:
        raise ValueError('later experiments must bind all earlier frozen panels for freshness')
    earlier_bodies = preceding_bodies(prior)
    if kind == 'overlap':
        previous = serial_basis(manifest)
        if identity(previous) not in {identity(unseal(v)) for v in prior}:
            raise ValueError('serial qualification manifest must also appear in the freshness chain')
    elif 'serial_qualification' in manifest:
        raise ValueError('serial qualification belongs only to the isolated overlap experiment')
    seen_hashes, seen_bodies = set(), set()
    for stage, cells in manifest['panels'].items():
        if not cells or len({c['id'] for c in cells}) != len(cells):
            raise ValueError(f'{stage}: require distinct nonempty cells')
        stage_hashes, stage_bodies = set(), set()
        tags = set()
        for cell in cells:
            if not sha(cell['case_sha256']) or not sha(cell.get('body_sha256')) or type(cell['threads']) is not int or not 1 <= cell['threads'] <= 1024:
                raise ValueError('case hash or worker count invalid')
            if 'memory_limit_bytes' in cell and (type(cell['memory_limit_bytes']) is not int or cell['memory_limit_bytes'] <= 0):
                raise ValueError('memory limit must be a positive integer')
            if not cell.get('family') or not cell.get('coverage'):
                raise ValueError('case family and coverage required')
            tags.update(cell['coverage'])
            stage_hashes.add(cell['case_sha256'])
            stage_bodies.add(cell['body_sha256'])
            if (kind == 'prefix' or COMPACT_IT in cell['comparators']):
                if (cell.get('categorical_intercepts') is not True or type(cell.get('absorbed_terms')) is not int
                        or cell['absorbed_terms'] < (3 if kind == 'prefix' else 1)):
                    raise ValueError('prefix/compact IT eligibility requires declared categorical intercept terms')
            comparators = cell['comparators']
            if len(comparators) != len(set(comparators)) or any(a not in arms or a in candidates or a in protections for a in comparators):
                raise ValueError('declare unique eligible explicit comparator arms')
        if earlier_bodies & stage_bodies:
            raise ValueError('later experiment reuses a prior frozen input/model body')
        if kind == 'overlap' and not any(c['threads'] >= 2 for c in cells):
            raise ValueError('overlap panel needs a declared eligible multiple-worker cell')
        if seen_hashes & stage_hashes or seen_bodies & stage_bodies:
            raise ValueError('A/B/C must not share case or numeric/model body hashes')
        seen_hashes.update(stage_hashes)
        seen_bodies.update(stage_bodies)
        required_coverage = (PREFIX_COVERAGE if kind == 'prefix' else
                             PORTFOLIO_COVERAGE if kind == 'portfolio' else
                             COVERAGE | (SAMPLED_COVERAGE if sampled else set()))
        if not ablation and not required_coverage <= tags:
            raise ValueError(f'{stage}: missing coverage {sorted(required_coverage - tags)}')
    return manifest


def required_candidates(manifest, stage, previous):
    if stage == 'A':
        if previous is not None:
            raise ValueError('A has no previous selection')
        return manifest['candidates']
    if previous is None or previous.get('stage') != ('A' if stage == 'B' else 'B'):
        raise ValueError('B/C requires its immediately preceding frozen selection')
    if previous.get('manifest_sha256') != identity(manifest):
        raise ValueError('selection belongs to a different frozen manifest')
    if stage == 'B':
        shortlist = previous['shortlist']
        if len(shortlist) > 4 or not shortlist or len(set(shortlist)) != len(shortlist) or any(a not in manifest['candidates'] for a in shortlist):
            raise ValueError('invalid A shortlist')
        return shortlist
    selected = previous.get('selected')
    if selected not in manifest['candidates'] or previous.get('status') != 'selected':
        raise ValueError('C requires a passing B policy; no runner-up may be substituted')
    return [selected]


def observations(manifest, stage, candidates, records, previous):
    if records.get('purpose') == 'ablation_only':
        raise ValueError('ablation observations cannot select or promote routing policy')
    if records.get('purpose') == 'scientific_prerequisites':
        raise ValueError('scientific prerequisites are not timing observations and cannot select routing policy')
    if records.get('status') not in {'complete', 'complete_with_failed_cells'}:
        raise ValueError('producer did not complete its frozen panel')
    if records.get('manifest_sha256') != identity(manifest) or records.get('stage') != stage:
        raise ValueError('records do not belong to this frozen panel')
    if records.get('experiment_kind', 'routing') != manifest.get('experiment_kind', 'routing'):
        raise ValueError('observation experiment kind changed')
    if records.get('failure_policy') != manifest['failure_policy']:
        raise ValueError('observation failure policy changed')
    prior_hash = identity(previous) if previous else None
    if records.get('selection_sha256') != prior_hash:
        raise ValueError('measurements must bind the selection frozen before this panel')
    if records.get('harness_sha256') != manifest['harness_sha256']:
        raise ValueError('harness identity changed')
    cells = {c['id']: c for c in manifest['panels'][stage]}
    required = {key: set(candidates + manifest['protections'] + c['comparators']) for key, c in cells.items()}
    expected_arms = set().union(*required.values())
    if records.get('artifacts') != {a: manifest['arms'][a]['sha256'] for a in sorted(expected_arms)}:
        raise ValueError('frozen artifact coverage or identity changed')
    if records.get('cases') != {c: value['case_sha256'] for c, value in cells.items()}:
        raise ValueError('frozen case coverage or identity changed')
    indexed = {}
    for row in records['records']:
        key = (row['cell'], row['arm'], row['repetition'])
        if (key in indexed or key[0] not in cells or key[1] not in required[key[0]]
                or type(key[2]) is not int or key[2] not in range(-1, REPETITIONS[stage])):
            raise ValueError('unexpected or duplicated observation')
        if row.get('status') not in {'passed', *FAILURES}:
            raise ValueError('unknown observation status')
        if row['status'] == 'passed' and (type(row.get('seconds')) not in (int, float)
                or not math.isfinite(row['seconds']) or row['seconds'] <= 0
                or not sha(row.get('science_sha256')) or not sha(row.get('witness_sha256'))):
            raise ValueError('passing observation requires positive time, repeat fingerprint and scientific witness')
        indexed[key] = {**row, 'verified': verified_record(row, manifest)}
    prerequisites = {}
    for row in records.get('prerequisites', []):
        key = (row['cell'], row['arm'])
        if key in prerequisites or key[0] not in required or key[1] not in required[key[0]]:
            raise ValueError('unexpected or duplicated prerequisite')
        prerequisites[key] = row
    times, outcomes, failures = {}, {}, []
    for cell, arms in required.items():
        for arm in sorted(arms):
            rows = [indexed.get((cell, arm, r)) for r in range(-1, REPETITIONS[stage])]
            outcome = (arm_outcome(manifest, rows, prerequisites[cell, arm]) if (cell, arm) in prerequisites
                       else {'status': 'blocked', 'reason': 'missing scientific prerequisite'})
            outcomes[cell, arm] = outcome
            times[cell, arm] = [r['seconds'] for r in rows[1:]] if outcome['status'] == 'passed' else None
            if outcome['status'] != 'passed':
                failures.append({'cell': cell, 'arm': arm, 'role': arm_role(manifest, arm), **outcome,
                                 'availability_loss': 'infinite' if outcome['status'] == 'unavailable' else 'unresolved'})
    return times, outcomes, failures


def evaluate(manifest, stage, records, previous=None):
    if manifest.get('purpose') == 'ablation_only':
        raise ValueError('ablation manifest cannot select or promote routing policy')
    validate_manifest(manifest)
    candidates = required_candidates(manifest, stage, previous)
    times, outcomes, failures = observations(manifest, stage, candidates, records, previous)
    blockers = [r for r in failures if r['role'] == 'required_reference'
                or (r['role'] == 'optional_comparator' and r['status'] != 'unavailable')]
    ranking = []
    for arm in candidates:
        violations, worst, ratios, comparisons = 0, None, [], []
        valid = not blockers and all(outcomes[c['id'], arm]['status'] == 'passed' for c in manifest['panels'][stage])
        finite_frontier, unavailable = [], []
        for cell in manifest['panels'][stage]:
            cid = cell['id']
            t, baseline = times[cid, arm], times[cid, 'baseline']
            ratios.append(statistics.median(c / b for c, b in zip(t, baseline)) if t and baseline else math.inf)
            for reference in dict.fromkeys(manifest['protections'] + cell['comparators']):
                b = times[cid, reference]
                protected = reference in manifest['protections']
                info = {'cell': cid, 'reference': reference, 'protection': protected}
                if t is not None and b is None and not protected and outcomes[cid, reference]['status'] == 'unavailable':
                    comparisons.append({**info, 'status': 'availability_dominance', 'pass': None,
                                        'runtime_identified': False,
                                        'reason': 'candidate passed; optional reference failed all-success reliability gate'})
                    unavailable.append(info)
                    continue
                if t is None or b is None:
                    passed = False
                    detail = {'status': 'blocked', 'runtime_identified': False}
                else:
                    detail = compare(b, t, relative_allowance=.05 if protected else .20)
                    bound = detail['median_ratio'] if stage == 'A' else detail['bootstrap95'][1]
                    normalized = (bound - 1) / (detail['upper_allowance'] - 1)
                    passed = normalized <= 1
                    detail.update(status='compared', runtime_identified=True)
                    if not protected:
                        worst = normalized if worst is None else max(worst, normalized)
                        finite_frontier.append(info)
                comparisons.append({**info, 'pass': passed, **detail})
                if protected:
                    violations += not passed
        geometric = math.exp(statistics.mean(map(math.log, ratios))) if all(math.isfinite(v) for v in ratios) else math.inf
        passes = valid and all(c['pass'] is not False for c in comparisons)
        ranking.append({'arm': arm, 'baseline_violations': violations,
                        'worst_internal_normalized': worst,
                        'geomean_baseline_ratio': geometric if math.isfinite(geometric) else None,
                        'pass': passes, 'data_valid': valid, 'comparisons': comparisons,
                        'frontier_coverage': {'finite_comparisons': finite_frontier,
                                              'unavailable_comparisons': unavailable,
                                              'complete_runtime_frontier': not unavailable and valid}})
    def finite(value):
        return math.inf if value is None else value
    if stage == 'A':
        ranking.sort(key=lambda r: (r['baseline_violations'], finite(r['worst_internal_normalized']),
                                    finite(r['geomean_baseline_ratio']), conservative(r['arm'])))
    else:
        ranking.sort(key=lambda r: (not r['pass'], finite(r['worst_internal_normalized']), conservative(r['arm'])))
    result = {'schema': 2, 'experiment_kind': manifest.get('experiment_kind', 'routing'),
              'manifest_sha256': identity(manifest), 'stage': stage,
              'records_sha256': identity(records), 'selection_sha256': identity(previous) if previous else None,
              'confidence': 'nominal 95% paired-median bootstrap per complete joint-success comparison; no simultaneous claim',
              'claim_scope': 'runtime frontier conditional on complete joint success; unavailable references have undefined runtime',
              'failure_policy': manifest['failure_policy'], 'blocking_references': blockers,
              'failures': failures, 'ranking': ranking}
    if stage == 'A':
        shortlist = [r['arm'] for r in ranking if r['data_valid']][:4]
        result.update(status='shortlisted' if shortlist else 'no_qualifying_policy', shortlist=shortlist)
    elif stage == 'B':
        chosen = next((r['arm'] for r in ranking if r['pass']), None)
        result.update(status='selected' if chosen else 'no_qualifying_policy', selected=chosen)
    else:
        passed = ranking[0]['pass']
        result.update(status='qualified' if passed else 'not_qualified', frozen_policy=candidates[0],
                      promoted=candidates[0] if passed else None,
                      stopping_rule='single frozen holdout; no runner-up and no automatic continuation')
    return result


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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    freeze = sub.add_parser('freeze')
    freeze.add_argument('--spec', type=Path, required=True)
    select = sub.add_parser('select')
    select.add_argument('--manifest', type=Path, required=True)
    select.add_argument('--stage', choices=REPETITIONS, required=True)
    select.add_argument('--records', type=Path, required=True)
    select.add_argument('--previous', type=Path)
    variant = sub.add_parser('prepare')
    variant.add_argument('--source', type=Path, required=True)
    variant.add_argument('--arm', required=True)
    variant.add_argument('--threads', type=int, required=True)
    for command in [freeze, select, variant]:
        command.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'prepare':
        output = scratch(args.output)
        output.parent.mkdir(parents=True, exist_ok=True)
        prepare(args.source, output, args.arm, args.threads)
        return 0
    if args.command == 'freeze':
        value = validate_manifest(read_json(args.spec))
    else:
        previous = unseal(read_json(args.previous)) if args.previous else None
        value = evaluate(unseal(read_json(args.manifest)), args.stage, read_json(args.records), previous)
    output = scratch(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('x') as stream:
        stream.write(json.dumps(seal(value), indent=2, allow_nan=False) + '\n')
    return 0 if value.get('status') not in {'no_qualifying_policy', 'not_qualified'} else 1


if __name__ == '__main__':
    raise SystemExit(main())
