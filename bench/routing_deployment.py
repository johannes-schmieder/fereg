#!/usr/bin/env python3
"""Prospective public-default deployment gate for one frozen serial policy.

This standalone schema is intentionally not accepted by routing_run.py yet.
It never estimates, times, opens case bodies, or modifies an existing manifest.
Reports must come from fresh non-C checks, not from this declaration tool.
They are hash-bound attestations: reviewers inspect the source diff and the
fresh check producer's actual outputs. Freezing this declaration in Git before
timing, and allowing only one activation/C attempt, are operational controls;
an exclusive output path or command-line flag alone cannot prove chronology.
"""
from __future__ import annotations

import argparse
import copy
from pathlib import Path

import routing
from native_io import digest

KIND = 'public_ols_auto_deployment_v1'
PUBLIC = 'public-auto'
CHECKS = ('full_suite', 'build_abi', 'independent_science', 'transactions_break', 'explicit_solvers_ppml_unchanged')
CONTRACT = {
    'activation': 'public OLS Auto wiring only; selected policy unchanged',
    'allowed_production_paths': ['crates/fereg-plugin/src/estimate.rs'],
    'equivalence': 'all original A/B cells; two fresh calls per artifact; exact science and solver signature',
    'public_invocation': {'solver': 'auto', 'injection': None},
    'holdout': 'one public-default C candidate; no post-C production change, retry, runner-up or continuation',
    'baseline_allowance': {'relative': .05, 'absolute_seconds': .001},
    'internal_allowance': {'relative': .20, 'absolute_seconds': .001},
    'incumbent_allowance': {'relative': .05, 'absolute_seconds': .001},
    'confidence': 'nominal 95% paired-median bootstrap per comparison',
    'bootstrap_draws': 10000, 'bootstrap_seed': 739391,
}


def require(value, message):
    if not value:
        raise ValueError(message)


def reference(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': digest(path)}


def read_bound(ref, *, json=False):
    require(isinstance(ref, dict) and set(ref) == {'path', 'sha256'}
            and isinstance(ref['path'], str) and routing.sha(ref['sha256']), 'invalid evidence reference')
    path = Path(ref['path'])
    require(digest(path) == ref['sha256'], 'evidence identity changed: ' + str(path))
    return routing.read_json(path) if json else ref


def origin(envelope, *, block_context=None):
    value = routing.unseal(envelope)
    routing.validate_manifest(value)
    require(value.get('experiment_kind', 'routing') in {'routing', 'portfolio', 'incremental'}
            and value.get('purpose') != 'ablation_only', 'deployment requires serial routing or the exact portfolio')
    incremental = value.get('experiment_kind') == 'incremental'
    if incremental:
        require(block_context is not None, 'incremental activation requires its validated Mac block context')
        import routing_blocks as blocks
        context, original = blocks.load_context(block_context)
        require(context.get('stage') == 'B' and context.get('experiment_kind') == 'incremental'
                and original == value, 'incremental activation requires its original B context')
        require(value['candidates'] == ['incumbent', routing.incremental_candidate(value)],
                'incremental activation changed family candidates')
    if value.get('experiment_kind') == 'portfolio':
        candidate = routing.PORTFOLIO_V1
        if block_context is not None:
            import routing_blocks as blocks
            context, original = blocks.load_context(block_context)
            require(context.get('kind') == blocks.KIND and context.get('stage') == 'B'
                    and original == value, 'deployment opt-in requires the validated original B context')
            candidate = routing.portfolio_candidate(value)
            plan = blocks.read(context['inputs']['discovery_plan'], sealed=True)
            require(candidate == routing.PORTFOLIO_V1 or plan.get('schema') == 3,
                    'new portfolio activation requires schema-3 discovery')
        require(value['candidates'] == ['baseline', candidate],
                'portfolio deployment requires its exact closed candidate order')
    require(PUBLIC not in value['arms'] and 'selected-hidden' not in value['arms'] and not any('incumbent' in cell['comparators']
            for cell in value['panels']['C']), 'reserved deployment arm collides with an original comparator')
    # Replacing a duplicate baseline loses no protected behavior. A distinct
    # incumbent needs a separately reviewed extension, never silent removal.
    if 'incumbent' in value['protections'] and not incremental:
        require(value['arms']['incumbent'] == value['arms']['baseline'],
                'cannot replace a distinct original protected incumbent')
    source = value.get('candidate_source_identity', {})
    candidates = {value['arms'][arm]['sha256'] for arm in value['candidates'] if arm not in (value['protections'] if incremental else ['baseline'])}
    require(all(routing.sha(source.get(key)) for key in
                ['plugin_sha256', 'production_tokens_sha256', 'artifact_inputs_sha256'])
            and candidates == {source['plugin_sha256']}, 'original candidate source/artifact identity is incomplete')
    return value


def non_c_cells(manifest):
    return [{key: cell[key] for key in ('id', 'case_sha256', 'body_sha256', 'threads')}
            | {'stage': stage} for stage in ['A', 'B'] for cell in manifest['panels'][stage]]


def declare(original):
    manifest = origin(original)
    return {'schema': 1, 'kind': KIND, 'status': 'prospective_before_any_timing',
            'original_manifest_sha256': routing.identity(manifest), 'contract': copy.deepcopy(CONTRACT),
            'non_c_cells': non_c_cells(manifest),
            'statistic_source_sha256': digest(Path(routing.__file__).with_name('time_exact.py')),
            'selector_source_sha256': digest(Path(routing.__file__)),
            'deployment_validator_sha256': digest(Path(__file__))}


def validate_declaration(envelope, original):
    value = routing.unseal(envelope)
    expected = declare(original)
    # Post-B producer hooks must preserve the original selector/statistic source.
    require(value == expected,
            'deployment amendment or fixed statistical contract changed')
    return value


def selected_b(original, a_decision, a_records, b_decision, b_records):
    a, b = routing.unseal(a_decision), routing.unseal(b_decision)
    require(a == routing.evaluate(original, 'A', a_records), 'A decision does not match its complete original records')
    require(b == routing.evaluate(original, 'B', b_records, a), 'B decision does not match its complete original records')
    winner = b.get('selected')
    require(b.get('status') == 'selected' and winner in original['candidates'] and winner != 'baseline',
            'public activation requires one passing nonbaseline B winner')
    require(routing.policy(winner) == original['arms'][winner]['policy'], 'B winner is not the original serial h/k policy')
    return winner, a, b


def activation_evidence(packet, declaration, manifest, selected):
    return activation_evidence_for_contract(packet, declaration, manifest, selected,
                                            kind=KIND, contract=CONTRACT)


def activation_evidence_for_contract(packet, declaration, manifest, selected, *, kind, contract, equivalence=True):
    """Shared nonstatistical checks; callers validate their own declaration/selection."""
    require(packet.get('schema') == 1 and packet.get('kind') == kind, 'invalid activation evidence schema')
    hidden = manifest['arms'][selected]['sha256']
    require(packet.get('selected') == selected and packet.get('hidden_sha256') == hidden,
            'activation evidence changed the selected hidden artifact')
    plugin = read_bound(packet['public_plugin'])
    require(plugin['sha256'] != hidden and plugin['sha256'] != manifest['arms']['baseline']['sha256'],
            'public activation requires its new frozen artifact')
    if manifest.get('experiment_kind') == 'incremental':
        require(plugin['sha256'] != manifest['arms']['incumbent']['sha256'],
                'incremental activation cannot reuse the prior public artifact')
    source = read_bound(packet['public_source'], json=True)
    require(source.get('plugin_sha256') == plugin['sha256'] and all(routing.sha(source.get(key)) for key in
            ['production_tokens_sha256', 'artifact_inputs_sha256', 'full_tests_log_sha256', 'native_build_log_sha256']),
            'new source/build/test identity is incomplete')
    require(source != manifest.get('candidate_source_identity'), 'activation reused the old source identity')
    reports = packet['reports']
    require(set(reports) == ({'source_review', 'correctness', 'equivalence'} if equivalence else
                            {'source_review', 'correctness'}), 'activation evidence is incomplete')
    common = {'status': 'passed', 'selected': selected, 'hidden_sha256': hidden,
              'public_sha256': plugin['sha256'], 'source_sha256': packet['public_source']['sha256'],
              'declaration_sha256': routing.identity(declaration)}
    values = {}
    for name, ref in reports.items():
        report = read_bound(ref, json=True)
        require(all(report.get(key) == value for key, value in common.items()), name + ': stale or failed activation report')
        require(report.get('kind') == kind + ':' + name, 'wrong activation report kind')
        values[name] = report
    review = values['source_review']
    require(review.get('activation_only') is True and review.get('public_invocation') == contract['public_invocation']
            and review.get('effective_policy') == manifest['arms'][selected]['policy'], 'activation changed solver semantics')
    changed = review.get('changed_production_paths')
    require(isinstance(changed, list) and changed and len(changed) == len(set(changed))
            and set(changed) <= set(contract['allowed_production_paths']), 'activation changes more than the permitted wiring')
    read_bound(review['independent_review'])
    # Canonical check_complexity.production_streams(root) maps, not the metrics
    # emitted by --write-inventory. A reviewer checks the actual wiring diff.
    inventories = review.get('production_inventory', {})
    require(set(inventories) == {'hidden', 'public'}, 'missing verified production diff')
    old = read_bound(inventories['hidden'], json=True)
    new = read_bound(inventories['public'], json=True)
    old_source = manifest.get('candidate_source_identity', {})
    require(old and set(old) == set(new) and routing.identity(old) == old_source.get('production_tokens_sha256')
            and routing.identity(new) == source['production_tokens_sha256'], 'production diff source identity mismatch')
    require(old_source.get('plugin_sha256') == hidden
            and source['artifact_inputs_sha256'] == old_source.get('artifact_inputs_sha256'),
            'activation changed build/dependency inputs or old source/artifact binding')
    require(sorted(key for key in old if old[key] != new[key]) == sorted(changed),
            'production diff exceeds reviewed activation wiring')
    correctness = values['correctness']
    require(set(correctness.get('checks', {})) == set(CHECKS), 'current correctness check missing')
    for name, check in correctness['checks'].items():
        require(check.get('status') == 'passed', 'current correctness failed: ' + name)
        read_bound(check['evidence'])
    require(correctness['checks']['full_suite']['evidence']['sha256'] == source['full_tests_log_sha256']
            and correctness['checks']['build_abi'].get('build_log_sha256') == source['native_build_log_sha256'],
            'correctness receipts do not match the new source build/tests')
    if not equivalence:
        return plugin
    equivalent = values['equivalence']
    require(equivalent.get('invocations') == {'hidden': {'solver': 'auto', 'injection': manifest['arms'][selected]['policy']['injection']},
            'public': contract['public_invocation']}, 'equivalence must compare hidden policy with uninjected public Auto')
    expected = {(cell['stage'], cell['id']): cell for cell in declaration['non_c_cells']}
    rows = equivalent.get('cells', [])
    require(len(rows) == len(expected) and {(r['stage'], r['id']) for r in rows} == set(expected),
            'equivalence must cover every predeclared non-C cell exactly once')
    forbidden = {c['body_sha256'] for c in manifest['panels']['C']}
    for row in rows:
        cell = expected[row['stage'], row['id']]
        require(all(row.get(key) == value for key, value in cell.items())
                and row['body_sha256'] not in forbidden, 'equivalence changed input/worker identity or examined C')
        calls = row.get('calls', [])
        require(len(calls) == 4 and {(r['arm'], r['repeat']) for r in calls} ==
                {(arm, repeat) for arm in ['hidden', 'public'] for repeat in [0, 1]}, 'equivalence needs two fresh calls per artifact')
        fingerprints = set()
        witnesses = set()
        for call in calls:
            require(call.get('status') == 'passed' and call.get('certificate_pass') is True
                    and call.get('science_pass') is True and routing.sha(call.get('science_sha256'))
                    and routing.sha(call.get('solver_signature_sha256')) and routing.sha(call.get('witness_sha256')),
                    'equivalence call lacks successful independent science')
            require(call.get('plugin_sha256') == (hidden if call['arm'] == 'hidden' else plugin['sha256']),
                    'equivalence call used another artifact')
            require(call.get('resource_checks') == {'iteration_caps': 'passed', 'memory_caps': 'passed', 'setup_caps': 'passed'},
                    'equivalence resource check missing or failed')
            read_bound(call['evidence'])
            fingerprints.add((call['science_sha256'], call['solver_signature_sha256']))
            witnesses.add(call['witness_sha256'])
        require(len(fingerprints) == 1, 'public/hidden science or solver signatures differ, or repeats are nonexact')
        require(len(witnesses) == 1, 'equivalence independent witness changed')
    return plugin


def derive(declaration, original, a_decision, a_records, b_decision, b_records, packet):
    manifest = origin(original)
    declared = validate_declaration(declaration, original)
    selected, a, b = selected_b(manifest, a_decision, a_records, b_decision, b_records)
    plugin = activation_evidence(packet, declared, manifest, selected)
    comparators = set().union(*(set(c['comparators']) for c in manifest['panels']['C']))
    arms = {name: copy.deepcopy(manifest['arms'][name]) for name in ['baseline', *sorted(comparators)]}
    arms['incumbent'] = dict(copy.deepcopy(manifest['arms'][selected]), role='required_reference')
    arms[PUBLIC] = {'sha256': plugin['sha256'], 'policy': None, 'role': 'candidate'}
    # A distinct schema prevents accidental consumption by the old hidden-arm
    # producer. Its post-B hook must validate this exact deterministic derivation.
    return {'schema': 1, 'kind': KIND, 'purpose': 'public_default_holdout', 'stage': 'C',
            'declaration': copy.deepcopy(declaration), 'original_manifest_sha256': routing.identity(manifest),
            'a_decision_sha256': routing.identity(a), 'a_records_sha256': routing.identity(a_records),
            'b_decision_sha256': routing.identity(b),
            'b_records_sha256': routing.identity(b_records), 'activation_evidence_sha256': routing.identity(packet),
            'selected': selected, 'effective_policy': copy.deepcopy(manifest['arms'][selected]['policy']),
            'public_invocation': copy.deepcopy(CONTRACT['public_invocation']), 'arms': arms,
            'candidates': [PUBLIC], 'protections': ['baseline', 'incumbent'],
            'panels': {'C': copy.deepcopy(manifest['panels']['C'])},
            'warmups': manifest['warmups'], 'repetitions': copy.deepcopy(manifest['repetitions']),
            'failure_policy': copy.deepcopy(manifest['failure_policy']), 'harness_sha256': manifest['harness_sha256'],
            'statistics': {key: copy.deepcopy(value) for key, value in CONTRACT.items() if key.endswith('allowance')
                           or key in {'confidence', 'bootstrap_draws', 'bootstrap_seed'}},
            'stopping_rule': CONTRACT['holdout']}


def validate_derived(envelope, *inputs):
    require(routing.unseal(envelope) == derive(*inputs), 'derived C manifest changed its frozen deployment contract')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    declaration = sub.add_parser('declare')
    declaration.add_argument('--before-any-timing', action='store_true', required=True)
    deployment = sub.add_parser('derive')
    for name in ['declaration', 'a-decision', 'a-records', 'b-decision', 'b-records', 'activation']:
        deployment.add_argument('--' + name, type=Path, required=True)
    for command in [declaration, deployment]:
        command.add_argument('--manifest', type=Path, required=True)
        command.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    original = routing.read_json(args.manifest)
    if args.command == 'declare':
        value = declare(original)
    else:
        read = lambda name: routing.read_json(getattr(args, name.replace('-', '_')))
        value = derive(read('declaration'), original, read('a-decision'), read('a-records'),
                       read('b-decision'), read('b-records'), read('activation'))
    output = routing.scratch(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('x') as stream:
        import json
        json.dump(routing.seal(value), stream, indent=2, allow_nan=False)
        stream.write('\n')


if __name__ == '__main__':
    main()
