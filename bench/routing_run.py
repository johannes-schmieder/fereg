#!/usr/bin/env python3
"""Produce frozen routing observations with the maintained native harness.

An untimed traced prerequisite verifies actual resource counters. Timed calls
have tracing disabled and must reproduce its exact science and solver signature.
External independent-witness, scoped-replay and resource-contract evidence is
mandatory and bound to the case and artifacts. This script never builds, retries,
resamples, chooses policies or weakens a failed scientific prerequisite.
"""
from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path
import platform
import shutil
import time

import numpy as np

import cmgschur
import routing_oracle
from replay_plugins import (allowed_difference, differences, exact_view, numerical_scope_indices,
                            scientific_errors, work_scope_indices)
from native_io import ROOT, case_header, digest
import routing
from routing_cases import inactive_case_oracle, inspect_case

class FrozenIdentityError(ValueError):
    pass


EXPLICIT = {'map': 'map', 'it': 'it', 'lsmr': 'lsmr', 'cmg': 'cmg', 'cmgschur': 'cmgschur5'}
CLEARED_ENV = ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING',
               'FEREG_CMG_ORDINARY', 'FEREG_ACTION_MEMORY_AUDIT', 'FEREG_PPML_MEMORY_TRACE',
               'FEREG_MEMORY_LIMIT_BYTES')


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


def generic_it_metadata(meta):
    # This metadata comes from inspect_case's canonical absorbed-term table,
    # not a family label, filename or retained rank. Categorical IT is distinct.
    return (isinstance(meta, dict) and meta.get('categorical_intercepts') is False
            and type(meta.get('absorbed_terms')) is int and meta['absorbed_terms'] > 0)


def generic_it_warning_limits(raw, meta):
    if not generic_it_metadata(meta):
        return None
    solver = raw['matrices']['RSOLVER']
    if (type(raw.get('return_code')) is not int or raw['return_code'] != 0
            or (solver['rows'], solver['columns'], len(solver['bits'])) != (2, 16, 32)):
        raise ValueError('explicit generic IT result/diagnostic shape changed')
    _, limits = cmg_polish_limits(raw, label='generic IT')
    # accelerated_map::report emits one informational warning on every generic
    # IT action, even initial certification with zero sweeps. Low concatenates
    # outcome/block warnings; final residual projection is one action in either
    # mode. Reuse the coefficient-capacity bound, never the retained rank.
    return limits[0], 1


def replay_scope(raw, arm, meta=None):
    view = exact_view(raw, False)
    numerical, work = numerical_scope_indices(False), work_scope_indices(False)
    scope = []
    for name, table in view['matrices'].items():
        indices = (range(len(table['bits'])) if name in {'RB', 'RV'} else
                   numerical.get(name, set()) | work.get(name, set()))
        scope.extend(f'/matrices/{name}/bits/{i}' for i in sorted(indices))
    scope.extend(f'/runtime_signature/{i}' for i in sorted(work.get('runtime_signature', set())))
    if arm != 'auto':
        rows = cmgschur.numbers(raw['matrices']['RSOLVER'])
        expected = cmgschur.EXPECTED_SOLVER[arm]
        cmg_mode, warning_limits = cmg_polish_limits(raw) if arm == 'cmg' else (None, None)
        it_limits = generic_it_warning_limits(raw, meta) if arm == 'it' else None
        for row in range(2):
            values = rows[row * 16:(row + 1) * 16]
            final_cmg_correction = arm == 'cmg' and row == 1 and values[1] == 4
            if final_cmg_correction:
                # CmgAbsorber::absorb_final_residual releases the hierarchy and
                # admits certified CG, which starts with no warnings; CMG then
                # adds exactly one correction warning. This is not public CG.
                fields = [0, 1, 3, 10, 11, 12, 13, 14, 15]
                if cmg_mode != 1 or [values[i] for i in fields] != [7, 4, 2, 1, 1, 0, 0, 0, 0]:
                    raise ValueError('explicit CMG final correction mapping changed')
            elif arm == 'cmg':
                fields = [0, 1, 3, 10, 12, 13, 14, 15]
                iterations, warnings = values[2], values[11]
                if ([values[i] for i in fields] != [7, 7, 2, 0, 0, 0, 0, 0]
                        or not math.isfinite(iterations) or iterations < 0 or not iterations.is_integer()
                        or not math.isfinite(warnings) or not 0 <= warnings <= warning_limits[row]
                        or not warnings.is_integer() or (warnings > 0 and iterations == 0)):
                    raise ValueError('explicit CMG polishing diagnostics changed')
            elif it_limits is not None:
                fields = [0, 1, 3, 10, 12, 13, 14, 15]
                iterations, warnings = values[2], values[11]
                if ([values[i] for i in fields] != [5, 5, 2, 0, 0, 0, 0, 0]
                        or not math.isfinite(iterations) or iterations < 0 or not iterations.is_integer()
                        or not math.isfinite(warnings) or not 1 <= warnings <= it_limits[row]
                        or not warnings.is_integer()):
                    raise ValueError('explicit generic IT informational diagnostics changed')
            elif (values[:2] != [expected, expected]
                    or values[12] != (1 if arm == 'lsmr' else 0)
                    or values[13] not in ({2, 3, 4} if arm == 'lsmr' else {0})):
                raise ValueError('explicit comparator engine/preconditioner mapping changed')
            # These request-dependent IDs are independently asserted above.
            # Auto IDs, tolerances, certificates and runtime signatures stay exact.
            scope.extend(f'/matrices/RSOLVER/bits/{row * 14 + i}' for i in [0, 1, 10, 11])
            if final_cmg_correction:
                # Only this typed final-correction tuple owns the two extra
                # leaves; ordinary CMG and every other route retain their scope.
                scope.extend(f'/matrices/RSOLVER/bits/{i}' for i in [22, 23])
            elif (arm == 'cmg' and warning_limits[row] > 0) or it_limits is not None:
                scope.append(f'/matrices/RSOLVER/bits/{row * 14 + 9}')
    return scope


def compatibility(reference, raw, reference_columns, columns, meta, arm):
    cmgschur.scientific_comparison(reference, raw, reference_columns, columns, meta)
    changed = differences(exact_view(reference, False), exact_view(raw, False))
    outside = [path for path in changed if not allowed_difference(path, replay_scope(raw, arm, meta))]
    if outside:
        raise ValueError('scoped replay changed an unaffected field: ' + '; '.join(outside[:5]))


def _explicit_worker_identity(source, cell, arm, artifact, *, prepared, solver, label):
    with source.open('rb') as stream:
        shape, arguments = case_header(stream)
    if not prepared:
        # Header-only projection of cmgschur.prepare's explicit options permits all
        # planned cells to be registered even when a witness is pending. The
        # actual prepared input must reproduce this identity before dispatch.
        updates = {'solver': solver, 'threads': str(cell['threads']), 'preconditioner': 'off'}
        arguments = [arg for arg in arguments if arg.split(b'=', 1)[0].decode()
                     not in {*updates, 'cmgdegree', 'cmgsamples', 'cmgseed'}]
        arguments += [f'{key}={value}'.encode() for key, value in updates.items()]
    threads = [arg for arg in arguments if arg.startswith(b'threads=')]
    if threads != [f"threads={cell['threads']}".encode()]:
        raise ValueError(f'{label} effective input does not bind its worker count')
    arguments = [arg for arg in arguments if not arg.startswith(b'threads=')]
    shape[2] = len(arguments)
    # Every non-thread option, including mode/pool/tolerance/injection, remains
    # part of the identity. The explicit memory limit is a native environment
    # option; all other solver environment variables are cleared by this runner.
    return routing.identity({'shape': shape, 'arguments': [arg.hex() for arg in arguments],
                             'body_sha256': cell['body_sha256'], 'arm': arm, 'artifact': artifact,
                             'memory_limit_bytes': cell.get('memory_limit_bytes')})


def cmg_worker_identity(source, cell, arm, artifact, *, prepared=True):
    return _explicit_worker_identity(source, cell, arm, artifact,
                                     prepared=prepared, solver='cmg', label='CMG')


def generic_it_worker_identity(source, cell, arm, artifact, *, prepared=True):
    return _explicit_worker_identity(source, cell, arm, artifact,
                                     prepared=prepared, solver='it', label='generic IT')


def cmg_diagnostic_signature(rows):
    if len(rows) != 2 or any(len(row) != 16 for row in rows):
        raise ValueError('missing CMG diagnostic rows')
    return [[row[i] for i in [0, 1, 3, 10, 11, 12, 13, 14, 15]] for row in rows]


def generic_it_diagnostic_signature(rows):
    if len(rows) != 2 or any(len(row) != 16 for row in rows):
        raise ValueError('missing generic IT diagnostic rows')
    # Cross-solver scope is irrelevant here: retain the raw warning count and
    # every discrete solver decision across same-request workers and repeats.
    return [[row[i] for i in [0, 1, 3, 10, 11, 12, 13, 14, 15]] for row in rows]


def cmg_worker_checks(result):
    return _diagnostic_worker_checks(result, 'cmg', cmg_diagnostic_signature, 'CMG')


def generic_it_worker_checks(result):
    return _diagnostic_worker_checks(result, 'generic_it', generic_it_diagnostic_signature, 'generic IT')


def _diagnostic_worker_checks(result, family, diagnostic_signature, label):
    groups = {}
    for request in result[family + '_worker_requests']:
        cid, arm = request['cell'], request['arm']
        before = [r for r in result['prerequisites'] if r['cell'] == cid and r['arm'] == arm]
        rows = [r for r in result['records'] if r['cell'] == cid and r['arm'] == arm]
        if len(before) != 1 or len(rows) != result['warmups'] + result['repetitions']:
            raise ValueError(f'{label} worker comparison has incomplete planned slots')
        complete = all(r.get('status') == 'passed' for r in [*before, *rows])
        signature = before[0].get(family + '_worker_signature') if complete else None
        if complete and (signature is None or any(diagnostic_signature(r['solver_rows']) != signature for r in rows)):
            raise ValueError(f'{label} exact repeat lost its diagnostic signature')
        groups.setdefault(request['identity'], []).append({**request, 'complete': complete, 'signature': signature})
    checks, blocked = [], set()
    for identity, members in sorted(groups.items()):
        members = sorted(members, key=lambda row: (row['threads'], row['cell'], row['arm']))
        available = [row for row in members if row['complete']]
        compared = len({row['threads'] for row in available}) > 1
        mismatch = compared and any(row['signature'] != available[0]['signature'] for row in available)
        if mismatch:
            blocked.update((row['cell'], row['arm']) for row in available)
        complete_coverage = len(available) == len(members)
        checks.append({'identity': identity,
                       'status': 'blocked' if mismatch else 'passed' if compared and complete_coverage else 'not_compared',
                       'complete_coverage': complete_coverage,
                       'compared_worker_counts': sorted({row['threads'] for row in available}) if compared else [],
                       'members': members})
    return checks, blocked


def publish_prerequisite(output, cell_index, proof, external, witness_path, no_fe):
    proof_path = output / f'proof-{cell_index}.json'
    proof_path.write_text(json.dumps(routing.seal(proof), indent=2, allow_nan=False) + '\n')
    link = {'path': proof_path.name, 'sha256': digest(proof_path)}
    wrapper = {key: proof[key] for key in ['schema', 'status', 'manifest_sha256', 'cell_id',
                  'case_sha256', 'body_sha256', 'threads', 'arms', 'arm_outcomes', 'stage', 'selection_sha256', 'frozen_dependencies']}
    independent = link
    if 'independent_witness' in external:
        original = external['independent_witness']
        independent = {'path': str((witness_path.parent / original['path']).resolve()), 'sha256': original['sha256']}
    wrapper.update(independent_witness=independent, scoped_replay=link, resource_contract=link)
    if no_fe:
        wrapper['no_fe_exact_path'] = link
    wrapper_path = output / f'witness-{cell_index}.json'
    wrapper_path.write_text(json.dumps(wrapper, indent=2, allow_nan=False) + '\n')
    return {'path': wrapper_path.name, 'sha256': digest(wrapper_path)}


def native_command(harness, plugin, source, target, cell, traced):
    command = ['env', *(value for name in CLEARED_ENV for value in ('-u', name))]
    if 'memory_limit_bytes' in cell:
        command.append(f"FEREG_MEMORY_LIMIT_BYTES={cell['memory_limit_bytes']}")
    if traced:
        command.append('FEREG_SOLVER_TRACE=1')
    return [*command, str(harness), str(plugin), '--replay', str(source), str(target)]


def input_policy(manifest, arm):
    if 'invocation' in manifest['arms'][arm]:
        if manifest['arms'][arm]['invocation'] != {'solver': 'auto', 'injection': None}:
            raise ValueError('public request must be uninjected Auto')
        return 'baseline'  # Request bytes only; effective policy is checked separately.
    if arm in manifest['candidates'] or arm in manifest['protections']:
        return routing.policy_name(manifest['arms'][arm].get('policy'))
    solver = manifest['arms'][arm].get('solver', arm)
    return solver if solver in routing.ABLATIONS else None


def reader_arm(manifest, arm):
    if 'invocation' in manifest['arms'][arm]:
        if manifest['arms'][arm].get('solver') != 'auto':
            raise ValueError('public reader must explicitly use Auto')
        input_policy(manifest, arm)
        return 'auto'
    return 'auto' if input_policy(manifest, arm) is not None else EXPLICIT[manifest['arms'][arm].get('solver', arm)]


def verify_public_request(source, manifest, arm):
    if 'invocation' in manifest['arms'][arm]:
        with source.open('rb') as stream:
            _, arguments = case_header(stream)
        if ([arg for arg in arguments if arg.startswith(b'solver=')] != [b'solver=auto']
                or any(arg.split(b'=', 1)[0] in {b'inject', b'break_marker_token'} for arg in arguments)):
            raise ValueError('prepared public Auto contains an injection or another solver')


def resource_policy(manifest, arm):
    config = manifest['arms'][arm]
    if 'invocation' in config:
        selected = routing.policy_name(config['effective_policy'])
        if selected == 'baseline' or not isinstance(config.get('trace_preset'), str) or not config['trace_preset']:
            raise ValueError('public Auto lacks its reviewed effective resource policy')
        return selected, config['trace_preset']
    return input_policy(manifest, arm), None


def paths(items):
    out = {}
    for item in items:
        name, path = item.split('=', 1)
        if not name or name in out:
            raise ValueError('duplicate or empty path label')
        out[name] = Path(path).resolve(strict=True)
    return out


def tool_identities():
    checkers = [ROOT / 'scripts' / name for name in
                ['replay_plugins.py', 'replay_psd.py', 'replay_bias_reference.py']]
    checkers.append(ROOT / 'crates/fereg-oracle/python/replay_ols_reference.py')
    producers = [Path(__file__).with_name(name) for name in
                 ['routing.py', 'routing_run.py', 'routing_cases.py', 'routing_oracle.py', 'cmgschur.py',
                  'cmgschur_cases.py', 'native.py', 'native_io.py', 'time_exact.py']]
    return {path: digest(path) for path in [*producers, *checkers]}


def evidence(path, cell, manifest, arms, *, prerequisites_only=False, local_oracle=False, stage='A', selection_sha256=None):
    value = routing.read_json(path)
    if (value.get('schema') != 2 or value.get('status') != ('ready' if prerequisites_only else 'checked')
            or value.get('case_sha256') != cell['case_sha256']
            or value.get('body_sha256') != cell['body_sha256']
            or value.get('manifest_sha256') != routing.identity(manifest)
            or value.get('cell_id') != cell['id'] or value.get('threads') != cell['threads']
            or any(value.get('arms', {}).get(a) != manifest['arms'][a] for a in arms)):
        raise ValueError('scientific evidence does not bind the passing case/artifacts')
    bound = {path: digest(path)}
    required = (['resource_contract', *([] if local_oracle else ['independent_witness'])]
                if prerequisites_only else ['independent_witness', 'scoped_replay', 'resource_contract'])
    for name in [*required, *(['independent_witness'] if local_oracle and 'independent_witness' in value else []),
                 *(['no_fe_exact_path'] if 'no_fe_exact_path' in value else [])]:
        proof = value[name]
        source = (path.parent / proof['path']).resolve(strict=True)
        if not routing.sha(proof['sha256']) or digest(source) != proof['sha256']:
            raise ValueError(f'{name}: evidence file hash mismatch')
        bound[source] = proof['sha256']
    dependencies = value.get('frozen_dependencies', [])
    if not isinstance(dependencies, list):
        raise ValueError('frozen dependencies must be a list of path/hash bindings')
    normalized_dependencies = []
    for dependency in dependencies:
        if not isinstance(dependency, dict) or set(dependency) != {'path', 'sha256'} or not isinstance(dependency['path'], str):
            raise ValueError('invalid frozen dependency binding')
        source = (path.parent / dependency['path']).resolve(strict=True)
        if not routing.sha(dependency['sha256']) or digest(source) != dependency['sha256']:
            raise ValueError('frozen dependency hash mismatch: ' + str(source))
        bound[source] = dependency['sha256']
        normalized_dependencies.append({'path': str(source), 'sha256': dependency['sha256']})
    if 'frozen_dependencies' in value:
        value['frozen_dependencies'] = normalized_dependencies
    if not prerequisites_only:
        if set(value['arms']) != set(arms):
            raise ValueError('prerequisite proof has unexpected or missing arms')
        if value.get('stage') != stage or value.get('selection_sha256') != selection_sha256:
            raise ValueError('prerequisite proof does not bind the frozen stage selection')
        proof = routing.unseal(routing.read_json((path.parent / value['scoped_replay']['path']).resolve()))
        for key in ['schema', 'status', 'manifest_sha256', 'cell_id', 'case_sha256', 'body_sha256', 'threads', 'arms', 'arm_outcomes', 'stage', 'selection_sha256', 'frozen_dependencies']:
            if proof.get(key) != value.get(key):
                raise ValueError('per-arm evidence disagrees with its sealed prerequisite proof')
        if proof.get('tool_sha256') != {p.name: v for p, v in tool_identities().items()}:
            raise ValueError('prerequisite proof used different producer or checkers')
        rows = [*proof['prerequisites'], *proof['repeat_checks']]
        if any(row.get('cell') != cell['id'] or row.get('arm') not in proof['arms'] for row in rows):
            raise ValueError('misassigned prerequisite proof row')
        prerequisite_outcomes(manifest, list(proof['arms']), proof['prerequisites'], proof['repeat_checks'])
        for row in rows:
            for artifact in row.get('failure_artifacts', {}).values():
                source = (path.parent / value['scoped_replay']['path']).resolve().parent / artifact['path']
                if digest(source) != artifact['sha256']:
                    raise ValueError('retained native failure artifact changed')
                bound[source] = artifact['sha256']
        outcomes = prerequisite_outcomes(manifest, arms, proof['prerequisites'], proof['repeat_checks'])
        if outcomes != {a: proof['arm_outcomes'].get(a) for a in arms} or not proof_admissible(manifest, outcomes):
            raise ValueError('invalid or blocking per-arm prerequisite outcomes')
    return value, bound


def prerequisite_outcomes(manifest, arms, prerequisites, repeats):
    outcomes = {}
    for arm in arms:
        before = [r for r in prerequisites if r['arm'] == arm]
        after = [r for r in repeats if r['arm'] == arm]
        if len(before) != 1 or len(after) != 1 or after[0].get('repetition') != 0:
            raise ValueError('prerequisite proof must retain exactly two planned calls per arm')
        first, second = before[0], after[0]
        checked = [{**r, 'verified': routing.verified_record(r, manifest)} for r in [first, second]]
        outcomes[arm] = routing.arm_outcome(manifest, checked[1:], checked[0])
    return outcomes


def proof_admissible(manifest, outcomes):
    return all(outcome['status'] == 'passed' if routing.arm_role(manifest, arm) == 'required_reference'
               else outcome['status'] in {'passed', 'unavailable'} if routing.arm_role(manifest, arm) == 'optional_comparator'
               else True for arm, outcome in outcomes.items())


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


def resource_diagnostics(log, requested, no_fe, witness, *, expected_preset=None):
    candidate = requested not in {None, 'baseline'}
    expected = ({'injection': routing.ABLATIONS[requested]} if requested in routing.ABLATIONS else
                routing.policy(requested) if candidate else None)
    actions, route = [], []
    for line in log.splitlines():
        if not line.startswith('FEREG_SOLVER_TRACE|'):
            continue
        fields = dict(part.split('=', 1) for part in line.split('|')[1:] if '=' in part)
        if fields.get('phase') == 'auto_schur_route':
            if expected and ('checkpoint' not in expected
                    or fields.get('checkpoint') != str(expected['checkpoint'])
                    or fields.get('samples') != str(expected['trees'])):
                raise ValueError('actual Schur route does not match requested policy')
            route.append(fields)
        if fields.get('phase') != 'routing_action':
            continue
        if expected and fields.get('preset') != (expected_preset if expected_preset is not None else expected['injection']):
            raise ValueError('routing_action does not attest the requested preset')
        if fields.get('final') not in {'0', '1'}:
            raise ValueError('routing_action lacks its projection/final action classification')
        action = {'final': fields.get('final'), 'preset': fields.get('preset')}
        for target, used, limit in [('iterations', 'iterations', 'native_limit'),
                                    ('setup_work', 'setup_work', 'setup_limit'),
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
    checks = {'iteration_caps': 'passed', 'memory_caps': 'passed',
              'setup_caps': 'passed' if candidate else 'not_applicable'}
    return {'action_resources': actions, 'resource_checks': checks, 'route_trace': route,
            'resource_basis': 'actual traced prerequisite' if actions else
                              'bound exact-no-FE witness' if candidate else 'bound legacy resource witness'}


def run(manifest, stage, previous, harness, plugins, cases, witnesses, output,
        timeout=120.0, budget=14400.0, ablation_only=False, prerequisites_only=False,
        qualification_context=None):
    # required_candidates enforces no A predecessor, frozen A shortlist for B,
    # and exactly the passing frozen B winner for C before reading any inputs.
    selection_sha256 = routing.identity(previous) if previous else None
    context_bindings = []
    if qualification_context is None:
        routing.validate_manifest(manifest)
        if manifest.get('purpose') == 'ablation_only' and not ablation_only:
            raise ValueError('ablation manifest requires --ablation-only collection')
        candidates = routing.required_candidates(manifest, stage, previous)
    else:
        if not prerequisites_only or ablation_only:
            raise ValueError('new qualification context is scientific-prerequisites-only')
        import routing_blocks
        qualified = routing_blocks.prerequisite_context(manifest, stage, previous, qualification_context)
        candidates = qualified['candidates']
        selection_sha256 = qualified['selection_sha256']
        context_bindings = qualified['bindings']
    if ablation_only:
        if manifest.get('experiment_kind', 'routing') != 'routing':
            raise ValueError('kernel collection cannot override a prefix or overlap experiment')
        if stage != 'A' or previous is not None:
            raise ValueError('standalone ablations use A with no previous selection')
        candidates = ['baseline']
    cells = manifest['panels'][stage]
    required = {c['id']: list(dict.fromkeys(['baseline', *manifest['protections'], *candidates, *c['comparators']]))
                for c in cells}
    arms = set().union(*(set(a) for a in required.values()))
    if (set(plugins) != arms or set(cases) != set(required)
            or (not set(witnesses) <= set(required) if prerequisites_only else set(witnesses) != set(required))):
        raise ValueError('plugin/case/witness labels must exactly cover the frozen stage')
    if timeout != manifest['failure_policy']['per_call_timeout_s'] or budget != manifest['failure_policy']['aggregate_budget_s']:
        raise ValueError('timeout and aggregate budget must equal the frozen failure policy')
    if any(digest(plugins[a]) != manifest['arms'][a]['sha256'] for a in arms):
        raise ValueError('arm artifact does not match its frozen identity')
    identities = {harness: manifest['harness_sha256']}
    identities.update({Path(ref['path']): ref['sha256'] for ref in context_bindings})
    identities.update({plugins[a]: manifest['arms'][a]['sha256'] for a in arms})
    identities.update({cases[c['id']]: c['case_sha256'] for c in cells})
    metadata, science, pending = {}, {}, {}
    for cell in cells:
        cid = cell['id']
        if digest(cases[cid]) != cell['case_sha256']:
            raise ValueError('frozen input changed')
        meta = inspect_case(cases[cid], cell['family'])
        if meta['body_sha256'] != cell['body_sha256']:
            raise ValueError('frozen numeric/model body changed')
        for key in ['absorbed_terms', 'categorical_intercepts']:
            if key in cell and meta.get(key) != cell[key]:
                raise ValueError('declared mathematical eligibility differs from the actual model')
        if (manifest.get('experiment_kind') == 'prefix' or routing.COMPACT_IT in required[cid]):
            if not meta.get('categorical_intercepts') or meta['absorbed_terms'] < (3 if manifest.get('experiment_kind') == 'prefix' else 1):
                raise ValueError('this arm requires categorical intercept fixed effects')
        if any(index < 0 or index >= meta['columns'] for index in meta['outputs'].values()):
            raise ValueError('routing qualification requires all canonical observation outputs')
        sidecar = cases[cid].with_suffix('.json')
        if sidecar.is_file():
            original = routing.read_json(sidecar)
            if original.get('case_sha256') != cell['case_sha256']:
                raise ValueError('case sidecar identity changed')
            if 'oracle' in original:
                meta['oracle'] = original['oracle']
            identities[sidecar] = digest(sidecar)
            if 'replay_reference' in original:
                ref = original['replay_reference']
                source = (sidecar.parent / ref['file']).resolve(strict=True)
                if digest(source) != ref['sha256']:
                    raise ValueError('imported independent replay reference changed')
                identities[source] = ref['sha256']
                meta['replay_reference'] = routing.read_json(source)
                if (meta['replay_reference'].get('saved', []) != meta.get('saved', [])
                        or meta['replay_reference'].get('outputs') != meta['outputs']):
                    raise ValueError('independent replay reference output mapping differs from the request')
        metadata[cid] = meta
        try:
            if cell['family'] == 'inactive-columns' and 'oracle' not in meta:
                meta['oracle'] = inactive_case_oracle(cases[cid])
            local_oracle = 'oracle' in meta or 'replay_reference' in meta
            if prerequisites_only and cid not in witnesses:
                raise ValueError('missing bound resource-contract evidence' if local_oracle else
                                 'missing case-specific independent witness and resource-contract evidence')
            science[cid], proof_hashes = evidence(witnesses[cid], cell, manifest, required[cid],
                prerequisites_only=prerequisites_only, local_oracle=local_oracle, stage=stage,
                selection_sha256=selection_sha256)
            identities.update(proof_hashes)
            independent = science[cid].get('independent_witness')
            if independent:
                source = (witnesses[cid].parent / independent['path']).resolve(strict=True)
                if source.suffix == '.json':
                    descriptor = routing.read_json(source)
                    if isinstance(descriptor, dict) and descriptor.get('kind') == routing_oracle.KIND:
                        meta['routing_oracle'], oracle_paths = routing_oracle.load_reference(
                            source, cell['case_sha256'], cell['body_sha256'])
                        meta['routing_oracle_sha256'] = digest(source)
                        identities.update(oracle_paths)
            if not any(key in meta for key in ['oracle', 'replay_reference', 'routing_oracle']):
                raise ValueError('missing executable case-specific independent truth')
            if prerequisites_only and meta['absorbed_terms'] == 0 and local_oracle:
                science[cid]['no_fe_exact_path'] = {'basis': 'canonical zero-FE model and independent native oracle/repeat'}
        except (KeyError, ValueError) as error:
            if not prerequisites_only:
                raise
            science[cid], pending[cid] = {}, str(error)
    tool_hashes = tool_identities()
    if any(path in identities and identities[path] != expected for path, expected in tool_hashes.items()):
        raise FrozenIdentityError('checker identity changed after frozen dependency validation')
    identities.update(tool_hashes)
    def unchanged():
        if any(digest(path) != expected for path, expected in identities.items()):
            raise ValueError('frozen artifact, harness, input, evidence or tooling changed')
    unchanged()
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    result = {'schema': 2, 'status': 'running', 'stage': stage,
              'experiment_kind': manifest.get('experiment_kind', 'routing'),
              'failure_policy': manifest['failure_policy'],
              'purpose': 'scientific_prerequisites' if prerequisites_only else 'ablation_only' if ablation_only else 'policy_tournament',
              'platform': platform.platform(), 'machine': platform.machine(),
              'cleared_environment': list(CLEARED_ENV), 'manifest_sha256': routing.identity(manifest),
              'selection_sha256': selection_sha256,
              'harness_sha256': manifest['harness_sha256'],
              'artifacts': {a: manifest['arms'][a]['sha256'] for a in sorted(arms)},
              'cases': {c['id']: c['case_sha256'] for c in cells},
              'tool_sha256': {p.name: v for p, v in tool_hashes.items()},
              'warmups': 0 if prerequisites_only else 1,
              'repetitions': 1 if prerequisites_only else routing.REPETITIONS[stage], 'timeout_s': timeout, 'budget_s': budget,
              'clock': ('scientific calls only; no timing observations or selection' if prerequisites_only else
                        'CLOCK_MONOTONIC around one complete native call; timed tracing disabled'),
              'stopping_rule': 'fixed panel, no retry or continuation; failed prerequisites block dependent timing',
              'prerequisites': [], 'prepared_sha256': {}, 'records': [], 'summary': [], 'qualified_wrappers': {},
              'cmg_worker_requests': [
                  {'cell': cell['id'], 'arm': arm, 'threads': cell['threads'],
                   'identity': cmg_worker_identity(cases[cell['id']], cell, arm, manifest['arms'][arm]['sha256'], prepared=False)}
                  for cell in cells for arm in required[cell['id']]
                  if input_policy(manifest, arm) is None and manifest['arms'][arm].get('solver', arm) == 'cmg'],
              'cmg_worker_checks': [],
              'generic_it_worker_requests': [
                  {'cell': cell['id'], 'arm': arm, 'threads': cell['threads'],
                   'identity': generic_it_worker_identity(cases[cell['id']], cell, arm, manifest['arms'][arm]['sha256'], prepared=False)}
                  for cell in cells for arm in required[cell['id']]
                  if input_policy(manifest, arm) is None and manifest['arms'][arm].get('solver', arm) == 'it'
                  and generic_it_metadata(metadata[cell['id']])],
              'generic_it_worker_checks': []}
    if qualification_context is not None:
        result['qualification_context'] = qualification_context
    pending_publications = []
    def save():
        (output / 'records.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')
    def execute(row, arm, source, target, log, cell, traced):
        left = budget - (time.monotonic() - started)
        row.update(dispatched=False, prepared_sha256=digest(source), call_limit_s=min(timeout, max(0.0, left)))
        if left <= 0:
            row.update(status='blocked', failure={'kind': 'aggregate_budget_unresolved', 'dispatched': False})
            return False
        row['dispatched'] = True
        save()
        began = time.monotonic()
        try:
            row.update(cmgschur.run_process(native_command(harness, plugins[arm], source, target, cell, traced),
                                           log, row['call_limit_s']))
            row['process_completed'] = True
            return True
        except (RuntimeError, TimeoutError, OSError) as error:
            failed_call(row, error, target, log, began, native=True)
            return False

    def failed_call(row, error, target, log, began=None, native=False):
        failure = {'kind': 'science_or_resource_failed', 'dispatched': row.get('dispatched', False),
                   'prepared_sha256': row.get('prepared_sha256'),
                   'process_completed': row.get('process_completed', False),
                   'call_limit_s': row.get('call_limit_s')}
        if log.exists():
            failure['log_sha256'] = digest(log)
            failure['trace'] = [line for line in log.read_text(errors='replace').splitlines()
                                if line.startswith('FEREG_SOLVER_TRACE|')]
        if native:
            failure['kind'] = 'infrastructure_unresolved'
        if isinstance(error, TimeoutError) and native and began is not None:
            failure['elapsed_s'] = time.monotonic() - began
            failure['kind'] = ('timeout' if row.get('call_limit_s') == timeout
                               else 'aggregate_budget_unresolved')
        elif target.exists() and row.get('process_completed'):
            # A return code alone is not availability. Only the predeclared
            # exact native diagnostics qualify, before any scientific checking.
            if native and target.stat().st_size <= 32 * 1024 * 1024:
                try:
                    raw = routing.read_json(target)
                    prefix = 'fereg: invalid_data: '
                    error_text = raw.get('error', '').strip()
                    message = error_text[len(prefix):] if error_text.startswith(prefix) else ''
                    if raw.get('schema') == 1 and raw.get('return_code') == 459 and message in routing.ITERATION_ERRORS:
                        failure.update(kind='iteration_exhausted', return_code=459, native_error=message,
                                       result_sha256=digest(target))
                    elif (raw.get('schema') == 1 and raw.get('return_code') == 430
                          and error_text == 'fereg: nonconvergence: ' + routing.CMGSCHUR_NONCONVERGENCE):
                        failure.update(kind='certification_nonconvergence', return_code=430,
                                       native_error_label='nonconvergence', native_error=routing.CMGSCHUR_NONCONVERGENCE,
                                       result_sha256=digest(target))
                except (ValueError, OSError):
                    pass
        # Even a legitimate exhaustion/timeout cannot excuse an observed cap
        # violation. Legacy arms may lack counters; never invent them on failure.
        failure['action_resources'] = []
        for line in failure.get('trace', []):
            fields = dict(part.split('=', 1) for part in line.split('|')[1:] if '=' in part)
            if fields.get('phase') != 'routing_action':
                continue
            action = {}
            try:
                for target_name, used, limit in [('iterations', 'iterations', 'native_limit'),
                                                ('setup_work', 'setup_work', 'setup_limit'),
                                                ('memory_bytes', 'memory_bytes', 'memory_limit')]:
                    action[target_name + '_used'] = int(fields[used])
                    action[target_name + '_limit'] = int(fields[limit])
                    if not 0 <= action[target_name + '_used'] <= action[target_name + '_limit']:
                        raise ValueError('failed call exceeded an action cap')
            except (KeyError, ValueError):
                failure['kind'] = 'resource_failed'
            failure['action_resources'].append(action)
        row.update(status='unavailable', failure=failure, error=str(error))
        if not routing.terminal_failure(row, manifest):
            row['status'] = 'blocked'
        # Keep actual failed metadata, row columns and log once. Planned calls
        # link this row and do not retry it; any incomplete counters stay explicit.
        evidence_id = len(result['prerequisites']) + len(result['records'])
        row['failure_artifacts'] = {}
        for kind, source in [('result', target), ('columns', target.with_suffix('.json.columns')), ('log', log)]:
            if source.exists():
                destination = output / f'failure-{evidence_id}-{kind}{source.suffix}'
                shutil.copyfile(source, destination)
                row['failure_artifacts'][kind] = {'path': destination.name, 'sha256': digest(destination)}

    try:
        for cell_index, cell in enumerate(cells):
            cid, meta = cell['id'], metadata[cell['id']]
            local_oracle = 'oracle' in meta or 'replay_reference' in meta or 'routing_oracle' in meta
            if cid in pending:
                for arm in required[cid]:
                    result['prerequisites'].append({'cell': cid, 'arm': arm, 'status': 'blocked', 'error': pending[cid]})
                    result['records'].append({'cell': cid, 'arm': arm, 'repetition': 0,
                                               'status': 'blocked', 'error': pending[cid]})
                save()
                continue
            reference, reference_columns = None, output / 'reference.columns'
            prepared, fingerprints, verified, failed = {}, {}, {}, {}
            for arm_index, arm in enumerate(required[cid]):
                source = output / f'input-{cell_index}-{arm_index}.case'
                requested = input_policy(manifest, arm)
                if requested is not None:
                    routing.prepare(cases[cid], source, requested, cell['threads'])
                    reader = reader_arm(manifest, arm)
                else:
                    solver = manifest['arms'][arm].get('solver', arm)
                    if solver not in EXPLICIT:
                        raise ValueError('unsupported explicit comparator in native routing producer')
                    reader = reader_arm(manifest, arm)
                    if arm == routing.COMPACT_IT:
                        routing.prepare(cases[cid], source, routing.COMPACT_IT, cell['threads'])
                    else:
                        cmgschur.prepare(cases[cid], source, reader, cell['threads'],
                                         samples=manifest['arms'][arm].get('cmgsamples'))
                verify_public_request(source, manifest, arm)
                prepared[arm] = (source, reader)
                result['prepared_sha256'][f'{cid}/{arm}'] = digest(source)
                if reader == 'cmg':
                    planned = next(request for request in result['cmg_worker_requests']
                                   if request['cell'] == cid and request['arm'] == arm)
                    actual = cmg_worker_identity(source, cell, arm, manifest['arms'][arm]['sha256'])
                    if actual != planned['identity']:
                        raise FrozenIdentityError('CMG effective request differs from its planned worker identity')
                if reader == 'it' and generic_it_metadata(meta):
                    planned = next(request for request in result['generic_it_worker_requests']
                                   if request['cell'] == cid and request['arm'] == arm)
                    actual = generic_it_worker_identity(source, cell, arm, manifest['arms'][arm]['sha256'])
                    if actual != planned['identity']:
                        raise FrozenIdentityError('generic IT effective request differs from its planned worker identity')
                prerequisite = {'cell': cid, 'arm': arm, 'prepared_sha256': digest(source)}
                result['prerequisites'].append(prerequisite)
                target, log = output / 'current.json', output / 'current.log'
                columns = target.with_suffix('.json.columns')
                try:
                    if not prerequisites_only and science[cid]['arm_outcomes'][arm]['status'] != 'passed':
                        prior_path = (witnesses[cid].parent / science[cid]['scoped_replay']['path']).resolve()
                        prior = routing.unseal(routing.read_json(prior_path))
                        first = next(r for r in prior['prerequisites'] if r['arm'] == arm)
                        repeat = next(r for r in prior['repeat_checks'] if r['arm'] == arm)
                        origin = first if first['status'] != 'passed' else repeat
                        if origin.get('prepared_sha256') != digest(source):
                            raise FrozenIdentityError('unavailable prerequisite effective input changed')
                        prerequisite.update(origin)
                        prerequisite.update(observation_source='prior_prerequisite',
                                            prior_proof={'path': str(prior_path), 'sha256': digest(prior_path)})
                        failed[arm] = prerequisite
                        continue
                    if arm != 'baseline' and reference is None:
                        raise ValueError('baseline scientific prerequisite failed')
                    if not execute(prerequisite, arm, source, target, log, cell, True):
                        failed[arm] = prerequisite
                        continue
                    try:
                        raw, detail = cmgschur.read_run(target, log, meta, reader)
                    except (ValueError, RuntimeError) as error:
                        failed_call(prerequisite, error, target, log, native=True)
                        failed[arm] = prerequisite
                        continue
                    if not prerequisites_only:
                        prior = routing.unseal(routing.read_json((witnesses[cid].parent / science[cid]['scoped_replay']['path']).resolve()))
                        origin = next(r for r in prior['prerequisites'] if r['arm'] == arm)
                        if origin.get('prepared_sha256') != digest(source):
                            raise FrozenIdentityError('prerequisite effective input changed')
                        if origin['science_sha256'] != detail['science_sha256']:
                            raise ValueError('native science differs from independently checked prerequisite cohort')
                    check_oracle(raw, columns, meta)
                    if reference is not None:
                        compatibility(reference, raw, reference_columns, columns, meta, reader)
                    effective, preset = resource_policy(manifest, arm)
                    actual = resource_diagnostics(log.read_text(), effective,
                        meta['absorbed_terms'] == 0, science[cid], expected_preset=preset)
                    fingerprints[arm] = detail['science_sha256']
                    verified[arm] = {**actual, 'diagnostic_sha256': routing.identity(actual),
                                     'witness_sha256': digest(witnesses[cid])}
                    if manifest.get('purpose') == 'activation_equivalence':
                        verified[arm]['solver_signature_sha256'] = routing.identity(raw['matrices']['RSOLVER'])
                    prerequisite.update(status='passed', certificate_pass=True, science_pass=True, science_sha256=fingerprints[arm], **verified[arm])
                    if reader == 'cmg':
                        prerequisite['cmg_worker_signature'] = cmg_diagnostic_signature(detail['solver_rows'])
                    if reader == 'it' and generic_it_metadata(meta):
                        prerequisite['generic_it_worker_signature'] = generic_it_diagnostic_signature(detail['solver_rows'])
                    if arm == 'baseline':
                        reference = raw
                        columns.rename(reference_columns)
                except FrozenIdentityError:
                    raise
                except (ValueError, RuntimeError, TimeoutError) as error:
                    failed_call(prerequisite, error, target, log)
                    failed[arm] = prerequisite
                finally:
                    target.unlink(missing_ok=True)
                    columns.unlink(missing_ok=True)
                    log.unlink(missing_ok=True)
                    save()
            for repetition in ([0] if prerequisites_only else range(-1, routing.REPETITIONS[stage])):
                for position, arm in enumerate(cmgschur.order(repetition, required[cid])):
                    row = {'cell': cid, 'arm': arm, 'repetition': repetition, 'order': position,
                           'prepared_sha256': result['prepared_sha256'][f'{cid}/{arm}']}
                    result['records'].append(row)
                    if arm in failed:
                        row.update(status='not_run', blocked_by_sha256=routing.observation_identity(failed[arm]),
                                   error=failed[arm].get('error', 'failed prerequisite'))
                        save()
                        continue
                    source, reader = prepared[arm]
                    target, log = output / 'current.json', output / 'current.log'
                    columns = target.with_suffix('.json.columns')
                    try:
                        if digest(source) != row['prepared_sha256']:
                            raise FrozenIdentityError('prepared arm input changed')
                        if not execute(row, arm, source, target, log, cell, False):
                            failed[arm] = row
                            continue
                        try:
                            raw, detail = cmgschur.read_run(target, log, meta, reader)
                        except (ValueError, RuntimeError) as error:
                            failed_call(row, error, target, log, native=True)
                            failed[arm] = row
                            continue
                        row.update(detail)
                        if detail['science_sha256'] != fingerprints[arm]:
                            raise ValueError('timed call differs from traced prerequisite science or solver signature')
                        check_oracle(raw, columns, meta)
                        compatibility(reference, raw, reference_columns, columns, meta, reader)
                        row.update(status='passed', certificate_pass=True, science_pass=True, **verified[arm])
                        if manifest.get('purpose') == 'activation_equivalence':
                            row['solver_signature_sha256'] = routing.identity(raw['matrices']['RSOLVER'])
                    except FrozenIdentityError:
                        raise
                    except (ValueError, RuntimeError, TimeoutError) as error:
                        failed_call(row, error, target, log)
                        failed[arm] = row
                    finally:
                        if prerequisites_only:
                            for key in ['seconds', 'wall_s', 'load_s', 'native_s', 'total_s', 'timing_field']:
                                row.pop(key, None)
                        target.unlink(missing_ok=True)
                        columns.unlink(missing_ok=True)
                        log.unlink(missing_ok=True)
                        save()
            if prerequisites_only:
                rows = [r for r in result['records'] if r['cell'] == cid]
                prerequisites = [r for r in result['prerequisites'] if r['cell'] == cid]
                outcomes = prerequisite_outcomes(manifest, required[cid], prerequisites, rows)
                if proof_admissible(manifest, outcomes):
                    unchanged()
                    proof = {'schema': 2, 'status': 'checked', 'arm_outcomes': outcomes, 'stage': stage,
                             'selection_sha256': selection_sha256, 'manifest_sha256': routing.identity(manifest),
                             'cell_id': cid, 'case_sha256': cell['case_sha256'], 'body_sha256': cell['body_sha256'],
                             'threads': cell['threads'], 'arms': {a: manifest['arms'][a] for a in required[cid]},
                             'replay_scope': {'numerical_scalar_indices': {k: sorted(v) for k, v in numerical_scope_indices(False).items()},
                                              'work_scalar_indices': {k: sorted(v) for k, v in work_scope_indices(False).items()},
                                              'dimension_dependent': ['RB/bits/<scalar>', 'RV/bits/<scalar>'],
                                              'explicit_comparator_mapping': 'explicit CMG7/7 certified stop2/fallback0/preconditioners0/pilot0 allows bounded polishing-warning leaves9/23: Fast primary<=1/final0, Low primary<=1+ceil(RSTATUS.rows/poolsize)/final<=1; positive warnings require positive iterations; only Fast final7/4 stop2/fallback1/warning1/preconditioners0/pilot0 permits final leaves22/23; other explicit mappings unchanged; CMG diagnostic tuples stay exact across repeats and comparable worker requests',
                                              'generic_it_comparator_mapping': 'only explicit IT on a parsed noncategorical absorbed model: requested/used5/5 certified stop2/fallback0/preconditioners0/pilot0 permits warning leaves9/23; one informational warning per action, including zero iterations; Fast primary1/final1, Low primary1..1+ceil(RSTATUS.rows/poolsize)/final1; all other fields retain scope and raw diagnostic tuples remain exact across repeats and comparable worker requests'},
                             'tool_sha256': result['tool_sha256'],
                             'independent_oracle_sha256': (meta['routing_oracle_sha256'] if 'routing_oracle' in meta else
                                 routing.identity(meta.get('oracle', meta.get('replay_reference'))) if local_oracle else None),
                             'external_evidence': science[cid],
                             'frozen_dependencies': science[cid].get('frozen_dependencies', []),
                             'evidence_source': {'path': str(witnesses[cid]), 'sha256': digest(witnesses[cid])},
                             'prerequisites': [r for r in result['prerequisites'] if r['cell'] == cid],
                             'repeat_checks': rows,
                             'scope': 'two planned scientific calls per arm; successful arms pass exact repeat, independent truth, baseline compatibility and resources; terminal failures retain availability only; no performance evidence'}
                    pending_publications.append((cell_index, cid, proof, science[cid], meta['absorbed_terms'] == 0))
                reference_columns.unlink(missing_ok=True)
                for source, _ in prepared.values():
                    source.unlink()
                unchanged()
                save()
                continue
            complete = {}
            for arm in required[cid]:
                observations = [r for r in result['records'] if r['cell'] == cid and r['arm'] == arm ]
                complete[arm] = ([r['seconds'] for r in observations if r['repetition'] >= 0] if len(observations) == routing.REPETITIONS[stage] + 1
                                 and all(r['status'] == 'passed' for r in observations) else None)
            summary = {'cell': cid, 'arms': {}}
            for arm, values in complete.items():
                arm_rows = [dict(r, verified=routing.verified_record(r, manifest)) for r in result['records']
                            if r['cell'] == cid and r['arm'] == arm]
                prerequisite = next(r for r in result['prerequisites'] if r['cell'] == cid and r['arm'] == arm)
                outcome = routing.arm_outcome(manifest, sorted(arm_rows, key=lambda r: r['repetition']), prerequisite)
                summary['arms'][arm] = ({'median_seconds': statistics.median(values),
                    'over_baseline': routing.compare(complete['baseline'], values) if complete['baseline'] else None}
                    if values else {'median_seconds': None, 'runtime_identified': False})
                summary['arms'][arm]['outcome'] = outcome
            result['summary'].append(summary)
            reference_columns.unlink(missing_ok=True)
            for source, _ in prepared.values():
                source.unlink()
            unchanged()
        unchanged()
        result['cmg_worker_checks'], blocked = cmg_worker_checks(result)
        result['generic_it_worker_checks'], it_blocked = generic_it_worker_checks(result)
        blocked |= it_blocked
        for row in [*result['prerequisites'], *result['records']]:
            if (row['cell'], row['arm']) in blocked and row.get('status') == 'passed':
                row.update(status='blocked', prior_status='passed',
                           error=('generic IT' if (row['cell'], row['arm']) in it_blocked else 'CMG') + ' diagnostics differ across comparable worker counts',
                           failure={'kind': 'cross_worker_discrete_mismatch'})
        for summary in result['summary']:
            for arm in summary['arms']:
                if (summary['cell'], arm) in blocked:
                    summary['arms'][arm] = {'median_seconds': None, 'runtime_identified': False,
                                           'outcome': {'status': 'blocked', 'reason': ('generic IT' if (summary['cell'], arm) in it_blocked else 'CMG') + ' cross-worker diagnostic mismatch'}}
        for cell_index, cid, proof, external, no_fe in pending_publications:
            if any(cell == cid for cell, _ in blocked):
                continue
            proof['cmg_worker_checks'] = [check for check in result['cmg_worker_checks']
                                         if any(member['cell'] == cid for member in check['members'])]
            proof['generic_it_worker_checks'] = [check for check in result['generic_it_worker_checks']
                                                if any(member['cell'] == cid for member in check['members'])]
            result['qualified_wrappers'][cid] = publish_prerequisite(
                output, cell_index, proof, external, witnesses[cid], no_fe)
        unchanged()
        result['status'] = ('complete_with_failed_cells' if any(r['status'] != 'passed' for r in result['records']) else 'complete')
    except BaseException as error:
        # Proof wrappers have no authority after any frozen identity failed,
        # including an identity changing during a later cell in this run.
        if prerequisites_only:
            for path in [*output.glob('witness-*.json'), *output.glob('proof-*.json')]:
                path.unlink()
            result['qualified_wrappers'] = {}
        result.update(status='failed', error=str(error))
        raise
    finally:
        result['elapsed_s'] = time.monotonic() - started
        save()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--stage', choices=routing.REPETITIONS, required=True)
    parser.add_argument('--previous', type=Path)
    parser.add_argument('--qualification-context', type=Path,
                        help='new Mac block context; scientific prerequisites only, without --previous')
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--plugin', action='append', required=True, help='arm=artifact')
    parser.add_argument('--case', action='append', required=True, help='cell=case')
    parser.add_argument('--witness', action='append', default=[], help='cell=evidence.json')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=float, default=120)
    parser.add_argument('--budget', type=float, default=14400)
    parser.add_argument('--ablation-only', action='store_true', help='collect baseline/comparators only; no policy promotion')
    parser.add_argument('--prerequisites-only', action='store_true', help='independent science/resource proofs for the frozen stage/selection; no timing observations or selection')
    args = parser.parse_args()
    context = None
    if args.qualification_context is not None:
        import routing_blocks
        context = routing_blocks.reference(args.qualification_context)
    result = run(routing.unseal(routing.read_json(args.manifest)), args.stage,
                 routing.unseal(routing.read_json(args.previous)) if args.previous else None,
                 args.harness.resolve(strict=True), paths(args.plugin), paths(args.case), paths(args.witness),
                 routing.scratch(args.output), args.timeout, args.budget, args.ablation_only, args.prerequisites_only,
                 qualification_context=context)
    return 0 if result['status'] == 'complete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
