#!/usr/bin/env python3
"""One public activation and paired C confirmation after the Mac block selector.

Only metadata/evidence are read here. The declaration is frozen before B; the
derived C context admits input inspection only after the actual B finalist and
fresh activation checks pass. Chronology and one-attempt enforcement also need
the retained operational review; a hash alone does not establish either.
"""
from __future__ import annotations

import copy
import hashlib
import json
import math
from pathlib import Path
import re

import routing
import routing_blocks as blocks
import routing_deployment as activation

KIND = 'routing_mac_public_blocks_v1'
EQUIVALENCE = KIND + '_equivalence'
PUBLIC = activation.PUBLIC
CONTRACT = {key: copy.deepcopy(activation.CONTRACT[key]) for key in [
    'activation', 'allowed_production_paths', 'equivalence', 'public_invocation',
    'holdout', 'baseline_allowance', 'internal_allowance', 'incumbent_allowance']}
CONTRACT.update(confidence='conditional marginal sign/order-statistic bounds, alpha=1/40 per tail',
                equivalence='all original A/B cells; two fresh calls per artifact; exact science and solver signature; first traced, second untraced exact repeat reuses prerequisite resource evidence',
                timing='24 separate two-arm ABBA/BAAB blocks per required C edge; no cross-edge call reuse')
INCREMENTAL_CONTRACT = {**copy.deepcopy(CONTRACT),
    'activation': 'one selected categorical OLS Auto/Auto family; preserve last accepted default outside family',
    'allowed_production_paths': ['crates/fereg-core/src/fe/session.rs'],
    'diagnostics': 'explicit solver science/availability retained; opportunity timings in SCC evidence; Mac timing contains protected Auto edges only',
    'ranking': 'equal-weight geometric mean of median block ratios to current Auto over affected-family cells; incumbent wins ties',
    'holdout': 'one frozen family activation and untouched C; failure or inconclusive restores prior default by ordinary commit; no post-C runner-up, retry or retuning'}


def contract_for(manifest):
    return INCREMENTAL_CONTRACT if manifest.get('experiment_kind') == 'incremental' else CONTRACT


INPUTS = {'declaration', 'b_config', 'b_decision', 'activation'}


def declare(b_context, design):
    """Prospective amendment; accepts no selected winner or C observations."""
    context, manifest = blocks.load_context(b_context)
    blocks.require(context['stage'] == 'B', 'declaration requires the prospective Mac B context')
    activation.origin(routing.seal(manifest), block_context=b_context)
    incremental = manifest.get('experiment_kind') == 'incremental'
    blocks.design_valid(design, attempts=24, incremental=incremental)
    if incremental:
        blocks.require(design['seed'] != context['design']['seed']
            and design['session_id'] != context['design']['session_id'],
            'incremental B/C need distinct frozen seeds and sessions')
    import time_blocks
    sources = [*time_blocks.source_bindings(), blocks.reference(blocks.__file__),
               blocks.reference(blocks.discovery.__file__), blocks.reference(activation.__file__),
               blocks.reference(__file__)]
    if manifest.get('experiment_kind') in {'portfolio', 'incremental'}:
        import routing_parallel
        sources.append(blocks.reference(routing_parallel.__file__))
    return {'schema': 1, 'kind': KIND + '_declaration', 'status': 'prospective_before_Mac_B',
            'b_context': b_context, 'original_manifest_sha256': routing.identity(manifest),
            'non_c_cells': activation.non_c_cells(manifest), 'contract': copy.deepcopy(contract_for(manifest)),
            'design': copy.deepcopy(design), 'sources': sources}


def activation_core(inputs, *, equivalence):
    """Validate selection/source/correctness; completed equivalence is a later gate."""
    blocks.require(set(inputs) == INPUTS, 'incomplete public activation inputs')
    declared = blocks.read(inputs['declaration'], sealed=True)
    blocks.require(declared == declare(declared['b_context'], declared['design']), 'public declaration changed')
    _, original = blocks.load_context(declared['b_context'])
    config = blocks.read(inputs['b_config'])
    decision = blocks.read(inputs['b_decision'], sealed=True)
    blocks.require(config['context'] == declared['b_context'], 'B used another prospective context')
    actual = routing.unseal(blocks.decide(config, decision['timing_manifest'],
                                        decision['records'], decision['monitor_receipt']))
    blocks.require(decision == actual, 'B decision differs from complete raw reanalysis')
    selected = decision.get('finalist')
    blocks.require(decision.get('kind') == blocks.KIND + '_B_decision'
        and decision.get('status') == 'selected' and selected in original['candidates']
        and selected not in original['protections'], 'activation requires the passing nonbaseline B finalist')
    contract = contract_for(original)
    packet = blocks.read(inputs['activation'])
    plugin = activation.activation_evidence_for_contract(packet, declared, original, selected,
        kind=KIND, contract=contract, equivalence=equivalence)
    review = blocks.read(packet['reports']['source_review'])
    trace = review.get('public_trace', {})
    blocks.require(set(trace) == {'preset', 'effective_policy'}
        and isinstance(trace['preset'], str) and re.fullmatch(r'[A-Za-z0-9_-]{1,100}', trace['preset'])
        and trace['effective_policy'] == original['arms'][selected]['policy'],
        'source review must bind public trace identity and selected effective policy')
    public = {'sha256': plugin['sha256'], 'role': 'candidate', 'solver': 'auto', 'policy': None,
        'invocation': copy.deepcopy(contract['public_invocation']),
        'effective_policy': copy.deepcopy(trace['effective_policy']), 'trace_preset': trace['preset']}
    return declared, original, selected, packet, trace, public


def equivalence_context(inputs, stage):
    """Exactly hidden/public A or B science; no C access and no timing selector."""
    blocks.require(stage in {'A', 'B'}, 'activation equivalence admits original A/B only')
    declared, original, selected, _, _, public = activation_core(inputs, equivalence=False)
    cells = [{**copy.deepcopy(cell), 'comparators': []} for cell in original['panels'][stage]]
    manifest = {'schema': 1, 'kind': EQUIVALENCE, 'purpose': 'activation_equivalence',
        'arms': {'baseline': {**copy.deepcopy(original['arms'][selected]), 'role': 'required_reference'}, PUBLIC: public},
        'execution_roles': {'baseline': 'hidden', PUBLIC: 'public'},
        'candidates': [PUBLIC], 'protections': ['baseline'], 'panels': {stage: cells},
        'failure_policy': copy.deepcopy(original['failure_policy']), 'harness_sha256': original['harness_sha256']}
    return {'schema': 1, 'kind': EQUIVALENCE, 'stage': stage, 'status': 'activation_equivalence',
        'inputs': copy.deepcopy(inputs), 'selected': selected, 'manifest': manifest,
        'cells': cells, 'candidates': [PUBLIC], 'protections': ['baseline'],
        'stopping': 'exactly two scientific calls per artifact/cell; no timing, C access or retries'}


def equivalence_calls(context_bound, records_bound):
    """Canonical report rows from actual producer records, not supplied assertions."""
    context, manifest, records = blocks.qualification(context_bound, records_bound)
    blocks.require(context['kind'] == EQUIVALENCE, 'equivalence requires its distinct A/B scientific context')
    cells, bindings = [], [context_bound, records_bound]
    for cell in context['cells']:
        cid = cell['id']
        wrapper = records['qualified_wrappers'][cid]
        witness = blocks.checked(records_bound).parent / wrapper['path']
        blocks.require(blocks.reference(witness)['sha256'] == wrapper['sha256'], 'equivalence wrapper changed')
        verified, dependencies = blocks.runner.evidence(witness, cell, manifest, ['baseline', PUBLIC],
            stage=context['stage'], selection_sha256=routing.identity(context))
        bindings += [blocks.reference(witness), *[blocks.reference(path) for path in dependencies]]
        before = [row for row in records['prerequisites'] if row['cell'] == cid]
        after = [row for row in records['records'] if row['cell'] == cid]
        proof_ref = verified['scoped_replay']
        proof_path = witness.parent / proof_ref['path']
        blocks.require(blocks.reference(proof_path)['sha256'] == proof_ref['sha256'], 'equivalence replay changed')
        proof = blocks.read(blocks.reference(proof_path), sealed=True)
        blocks.require(proof['prerequisites'] == before and proof['repeat_checks'] == after,
                       'equivalence records differ from independently checked wrapper')
        bindings.append(blocks.reference(proof_path))
        outcomes = blocks.runner.prerequisite_outcomes(manifest, ['baseline', PUBLIC], before, after)
        blocks.require(all(value['status'] == 'passed' for value in outcomes.values()), 'activation equivalence failed')
        calls = []
        for arm in ['baseline', PUBLIC]:
            for repeat, rows in enumerate([before, after]):
                row = next(row for row in rows if row['arm'] == arm)
                blocks.require(routing.sha(row.get('solver_signature_sha256')), 'equivalence lacks actual solver signature')
                calls.append({**{key: row[key] for key in ['status', 'certificate_pass', 'science_pass',
                    'science_sha256', 'solver_signature_sha256', 'witness_sha256', 'resource_checks', 'resource_basis']},
                    'arm': manifest['execution_roles'][arm], 'repeat': repeat,
                    'resource_evidence_repeat': 0,
                    'plugin_sha256': manifest['arms'][arm]['sha256'], 'evidence': records_bound})
        blocks.require(len({(call['science_sha256'], call['solver_signature_sha256']) for call in calls}) == 1,
                       'public/hidden exact science or solver signatures differ')
        cells.append({**{key: cell[key] for key in ['id', 'case_sha256', 'body_sha256', 'threads']},
                      'stage': context['stage'], 'calls': calls})
    return context, cells, bindings


def derive(inputs):
    """Recompute B and completed activation before admitting original C inputs."""
    declared, original, selected, packet, trace, public = activation_core(inputs, equivalence=True)
    equivalent = blocks.read(packet['reports']['equivalence'])
    blocks.require(equivalent.get('public_trace') == trace,
                   'fresh equivalence must attest the reviewed public trace policy')
    collections = equivalent.get('collections', [])
    blocks.require(len(collections) == 2, 'equivalence needs actual A and B producer records')
    actual, stages = [], set()
    for collection in collections:
        context, cells, _ = equivalence_calls(collection['context'], collection['records'])
        blocks.require(context['stage'] not in stages and all(context['inputs'][key] == inputs[key]
            for key in ['declaration', 'b_config', 'b_decision']), 'equivalence used another selection or repeated stage')
        partial = copy.deepcopy(packet)
        del partial['reports']['equivalence']
        blocks.require(blocks.read(context['inputs']['activation']) == partial, 'equivalence used another activation')
        stages.add(context['stage']); actual.extend(cells)
    blocks.require(stages == {'A', 'B'} and sorted(actual, key=lambda row: (row['stage'], row['id'])) ==
        sorted(equivalent['cells'], key=lambda row: (row['stage'], row['id'])),
        'equivalence report differs from its actual complete producer records')
    manifest = holdout_manifest(original, selected, public)
    incremental = original.get('experiment_kind') == 'incremental'
    return {'schema': 1, 'kind': KIND, 'stage': 'C', 'status': 'activated', 'inputs': copy.deepcopy(inputs),
        **({'experiment_kind': 'incremental'} if incremental else {}),
        'selected': selected, 'manifest': manifest, 'design': copy.deepcopy(declared['design']),
        'cells': manifest['panels']['C'], 'candidates': [PUBLIC], 'protections': manifest['protections'],
        'allowances': {'protected': {'relative': .05, 'absolute_seconds': .001},
                       'internal': {'relative': .20, 'absolute_seconds': .001}},
        'stopping': contract_for(original)['holdout']}


def holdout_manifest(original, selected, public):
    """Preserve accepted Auto separately from the selected hidden overhead control."""
    comparators = sorted(set().union(*(set(cell['comparators']) for cell in original['panels']['C'])))
    incremental = original.get('experiment_kind') == 'incremental'
    protections = ['baseline', 'incumbent', 'selected-hidden'] if incremental else ['baseline', 'incumbent']
    arms = {arm: copy.deepcopy(original['arms'][arm]) for arm in
            [*original['protections'], *comparators] if arm in original['arms']}
    arms['selected-hidden' if incremental else 'incumbent'] = {
        **copy.deepcopy(original['arms'][selected]), 'role': 'required_reference'}
    arms[PUBLIC] = public
    manifest = {'schema': 1, 'kind': KIND, 'purpose': 'public_default_holdout',
        **({'experiment_kind': 'incremental', 'incremental': copy.deepcopy(original['incremental'])} if incremental else {}),
        'arms': arms, 'candidates': [PUBLIC], 'protections': protections,
        'panels': {'C': copy.deepcopy(original['panels']['C'])},
        'failure_policy': copy.deepcopy(original['failure_policy']), 'harness_sha256': original['harness_sha256']}
    return manifest


def context_bindings(bound, context):
    inputs = context['inputs']
    declared = blocks.read(inputs['declaration'], sealed=True)
    b_context, _ = blocks.load_context(declared['b_context'])
    decision = blocks.read(inputs['b_decision'], sealed=True)
    import time_blocks
    frozen, _ = time_blocks.load_manifest(blocks.checked(decision['timing_manifest']))
    packet = blocks.read(inputs['activation'])
    reports = {key: blocks.read(value) for key, value in packet['reports'].items()}
    review, correctness = (reports[key] for key in ['source_review', 'correctness'])
    equivalent = reports.get('equivalence', {})
    values = [bound, *inputs.values(), *declared['sources'],
        *blocks.context_bindings(declared['b_context'], b_context),
        *[decision[key] for key in ['qualification', 'timing_manifest', 'records', 'monitor_receipt'] if decision[key]],
        *frozen['bindings'], packet['public_plugin'], packet['public_source'], *packet['reports'].values(),
        review['independent_review'], *review['production_inventory'].values(),
        *[check['evidence'] for check in correctness['checks'].values()],
        *[call['evidence'] for cell in equivalent.get('cells', []) for call in cell['calls']]]
    for collection in equivalent.get('collections', []):
        cohort, _, dependencies = equivalence_calls(collection['context'], collection['records'])
        values += [*dependencies, *context_bindings(collection['context'], cohort)]
    for value in values:
        blocks.checked(value)
    return list({value['path']: value for value in values}.values())


def timing_spec(config):
    return blocks.timing_spec(config, paired=True)


def decide(config, timing_manifest, records, monitor_receipt):
    """One final conditional gate; never select a fallback or admit more calls."""
    import time_blocks
    context, manifest = blocks.load_context(config['context'])
    blocks.require(context['kind'] == KIND and context['stage'] == 'C', 'final gate requires activated C context')
    expected = timing_spec(config)
    frozen, _ = time_blocks.load_manifest(blocks.checked(timing_manifest))
    blocks.require(frozen['spec'] == expected, 'timing manifest differs from complete paired C design')
    analysis = time_blocks.analyze(blocks.checked(timing_manifest), blocks.checked(records),
                                  blocks.checked(monitor_receipt) if monitor_receipt else None)
    coverage = blocks.comparison_coverage(config['context'], config['qualification'])
    pairs = {cell['id']: cell['comparisons'][0] for cell in expected['cells']}
    summaries = {}
    for row in analysis['summary']:
        cid = row.get('cell_id')
        blocks.require(cid in pairs and cid not in summaries and all(row.get(key) == value for key, value in {
            'comparison_id': pairs[cid]['id'], 'candidate_arm': PUBLIC,
            'reference_arm': pairs[cid]['reference'], 'reference_role': pairs[cid]['reference_role']}.items()),
            'unexpected, duplicated or changed C comparison')
        summaries[cid] = row
    passed = not analysis['missing_monitor_evidence'] and not analysis['scientific_or_execution_failure']
    comparisons = []
    for cell in expected['cells']:
        row = summaries.get(cell['id'])
        interval = row.get('excess_median_interval') if row else None
        upper = interval.get('upper') if interval else None
        normalized = row.get('normalized_regret_median_interval') if row else None
        normalized_upper = normalized.get('upper') if normalized else None
        ok = bool(row and row['conditional_nonregression_pass'] and row['attempted_slots'] == 24
            and upper is not None and math.isfinite(upper) and upper <= 0
            and normalized_upper is not None and math.isfinite(normalized_upper) and normalized_upper <= 1)
        if manifest.get('experiment_kind') != 'incremental' or pairs[cell['id']]['reference_role'] == 'protected':
            passed &= ok
        comparisons.append({'cell': cell['workload'], **pairs[cell['id']], 'pass': ok, 'analysis': row})
    return routing.seal({'schema': 1, 'kind': KIND + '_C_decision',
        'status': 'conditional_pass' if passed else 'not_confirmed',
        'conditional_nonregression_pass': bool(passed), 'promotion': False, 'qualification_pass': False,
        'public_sha256': manifest['arms'][PUBLIC]['sha256'], 'context': config['context'],
        'qualification': config['qualification'], 'timing_manifest': timing_manifest, 'records': records,
        'monitor_receipt': monitor_receipt, 'analysis_sha256': routing.identity(analysis),
        'comparisons': comparisons, 'unavailable': [edge for edge in coverage['comparisons'] if edge['status'] == 'unavailable'],
        'sources': [blocks.reference(__file__), blocks.reference(time_blocks.__file__)],
        'claim': 'All identified required edges pass conditional marginal bounds; no simultaneous confidence claim. Promotion requires operational review.',
        'stopping': contract_for(manifest)['holdout']})


TERMINAL = 'routing_incremental_stage_terminal'


def terminal_stage(inputs):
    """Bind the operational acceptance/restore review to rederived B or C evidence."""
    discovery_only = set(inputs) == {'discovery', 'operational_review'}
    if discovery_only:
        original, decision = blocks.discovery.terminal_basis(inputs['discovery'])
        manifest, context = original, None
    else:
        blocks.require(set(inputs) == {'config', 'decision', 'operational_review'},
                       'terminal stage requires actual config, decision and operational review')
        config = blocks.read(inputs['config'])
        context, manifest = blocks.load_context(config['context'])
        blocks.require(manifest.get('experiment_kind') == 'incremental',
                       'terminal receipt requires an incremental campaign')
        decision = blocks.read(inputs['decision'], sealed=True)
        selector = blocks.decide if context['stage'] == 'B' else decide
        actual = routing.unseal(selector(config, decision['timing_manifest'],
                                         decision['records'], decision['monitor_receipt']))
        blocks.require(decision == actual, 'terminal stage decision differs from raw reanalysis')
    review = blocks.read(inputs['operational_review'])
    status = review.get('outcome')
    blocks.require(review.get('kind') == TERMINAL + '_operational_review'
        and review.get('status') == 'passed' and status in {'accepted', 'retained'}
        and review.get('source_decision_sha256') == routing.identity(decision),
        'terminal outcome requires a passing decision-bound operational review')
    if discovery_only:
        blocks.require(status == 'retained', 'discovery never activates a public default')
    elif context['stage'] == 'B':
        blocks.require(status == 'retained' and decision.get('finalist') in {None, 'incumbent'},
                       'a selected challenger must complete activation/C before a terminal outcome')
        original = manifest
    else:
        declared = blocks.read(context['inputs']['declaration'], sealed=True)
        _, original = blocks.load_context(declared['b_context'])
        blocks.require(status != 'accepted' or decision.get('conditional_nonregression_pass') is True,
                       'failed or inconclusive C cannot accept a default')
    expected_artifact = manifest['arms'][PUBLIC if status == 'accepted' else 'incumbent']['sha256']
    source = blocks.read(review['incumbent_source'])
    revision = source.get('revision')
    artifacts = source.get('artifacts', {})
    blocks.require(isinstance(revision, str) and re.fullmatch('[0-9a-f]{40}', revision)
        and set(artifacts) == {'Darwin', 'Linux'}
        and (expected_artifact in artifacts.values() if discovery_only else artifacts['Darwin'] == expected_artifact)
        and all(routing.sha(value) for value in artifacts.values())
        and all(routing.sha(source.get(key)) for key in ['production_tokens_sha256', 'artifact_inputs_sha256']),
        'accepted/retained source must bind both platform artifacts and production/build inputs')
    if status == 'accepted':
        packet = blocks.read(context['inputs']['activation'])
        public_source = blocks.read(packet['public_source'])
        blocks.require(all(source[key] == public_source[key]
            for key in ['production_tokens_sha256', 'artifact_inputs_sha256']),
            'terminal accepted source differs from the confirmed public artifact')
    else:
        blocks.require(revision == original['incremental']['incumbent_revision'],
                       'retained outcome changed the prior accepted revision')
    evidence = review.get('evidence')
    builds = review.get('platform_builds', {})
    blocks.require(isinstance(evidence, list) and evidence and set(builds) == {'Darwin', 'Linux'},
                   'terminal operational review must retain actual platform build evidence')
    for host, bound in builds.items():
        build = blocks.read(bound)
        blocks.require(build.get('status') == 'passed' and build.get('platform') == host
            and build.get('revision') == revision and build.get('plugin_sha256') == artifacts[host]
            and all(build.get(key) == source[key]
                    for key in ['production_tokens_sha256', 'artifact_inputs_sha256']),
            'platform incumbent build differs from accepted source or artifact')
        blocks.checked(build['evidence'])
    for bound in [review['incumbent_source'], *evidence]:
        blocks.checked(bound)
    exported = blocks.read(review['independent_export_review'])
    export_expected = {'kind': TERMINAL + '_export_review', 'status': 'passed',
        'source_decision_sha256': routing.identity(decision),
        'incumbent_source_sha256': review['incumbent_source']['sha256'],
        'stage_family': original['incremental']['family'], 'outcome': status,
        'prior_manifest_sha256': routing.identity(original),
        'incumbent_revision': revision, 'incumbent_artifacts': artifacts,
        'incumbent_source_identity': {key: source[key] for key in
            ['production_tokens_sha256', 'artifact_inputs_sha256']}}
    blocks.require(all(exported.get(key) == value for key, value in export_expected.items())
        and isinstance(exported.get('evidence'), list) and exported['evidence'],
        'terminal export requires independent review of its actual decision and platform source evidence')
    for bound in exported['evidence']:
        blocks.checked(bound)
    return {'schema': 1, 'kind': TERMINAL, 'stage_family': original['incremental']['family'],
        'status': status, 'source_decision_sha256': routing.identity(decision),
        'incumbent_revision': revision, 'incumbent_artifacts': artifacts,
        'incumbent_source_identity': {key: source[key] for key in
            ['production_tokens_sha256', 'artifact_inputs_sha256']},
        'incumbent_source_sha256': review['incumbent_source']['sha256'],
        'export_review': {'binding': review['independent_export_review'], 'report': exported,
                          'text': blocks.checked(review['independent_export_review']).read_text()},
        'prior_manifest': routing.seal(original), 'inputs': copy.deepcopy(inputs)}


def validate_terminal(value, *, portable=False):
    """Portable checks bind an exported attestation; only local replay verifies raw evidence."""
    blocks.require(isinstance(value, dict) and set(value) == {'schema', 'kind', 'stage_family', 'status',
        'source_decision_sha256', 'incumbent_revision', 'incumbent_artifacts', 'incumbent_source_identity',
        'incumbent_source_sha256', 'export_review', 'prior_manifest', 'inputs'}
        and type(value['schema']) is int and value['schema'] == 1 and value['kind'] == TERMINAL
        and value['stage_family'] in {'many', 'two'} and value['status'] in {'accepted', 'retained'}
        and isinstance(value['incumbent_revision'], str) and re.fullmatch('[0-9a-f]{40}', value['incumbent_revision'])
        and routing.sha(value['source_decision_sha256']) and routing.sha(value['incumbent_source_sha256'])
        and set(value['incumbent_artifacts']) == {'Darwin', 'Linux'}
        and all(routing.sha(v) for v in value['incumbent_artifacts'].values())
        and set(value['incumbent_source_identity']) == {'production_tokens_sha256', 'artifact_inputs_sha256'}
        and all(routing.sha(v) for v in value['incumbent_source_identity'].values()),
        'invalid portable terminal metadata')
    prior = routing.unseal(value['prior_manifest'])
    routing.validate_manifest(prior)
    blocks.require(prior.get('experiment_kind') == 'incremental'
        and prior['incremental']['family'] == value['stage_family'],
        'terminal prior manifest changed family or experiment')
    exported = value['export_review']
    blocks.require(isinstance(exported, dict) and set(exported) == {'binding', 'report', 'text'}
        and isinstance(exported['binding'], dict) and set(exported['binding']) == {'path', 'sha256'}
        and routing.sha(exported['binding']['sha256'])
        and isinstance(exported['text'], str) and len(exported['text']) <= 262144
        and hashlib.sha256(exported['text'].encode()).hexdigest() == exported['binding']['sha256']
        and json.loads(exported['text']) == exported['report']
        and all(exported['report'].get(key) == expected for key, expected in {
            'kind': TERMINAL + '_export_review', 'status': 'passed',
            'source_decision_sha256': value['source_decision_sha256'],
            'incumbent_source_sha256': value['incumbent_source_sha256'],
            'stage_family': value['stage_family'], 'outcome': value['status'],
            'prior_manifest_sha256': routing.identity(prior),
            'incumbent_revision': value['incumbent_revision'],
            'incumbent_artifacts': value['incumbent_artifacts'],
            'incumbent_source_identity': value['incumbent_source_identity']}.items())
        and isinstance(exported['report'].get('evidence'), list) and exported['report']['evidence'],
        'portable terminal lacks its matching independent export review')
    if not portable:
        blocks.require(value == terminal_stage(value['inputs']),
                       'terminal stage changed its bound outcome or operational evidence')
    return value
