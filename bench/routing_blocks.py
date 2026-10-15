#!/usr/bin/env python3
"""Versioned SCC-discovery to Mac-block scientific context.

This coordinator never reads a case body or dispatches native work. The initial
stage is the original B panel, selected by reviewed SCC discovery, not a legacy
A decision. Fresh Mac prerequisites precede timing. C requires the separately
validated public-activation derivation before its inputs can be inspected.
Hash-bound review reports retain the actual remote validation output and raw
evidence; their hashes establish provenance, not the truth of the review.
"""
from __future__ import annotations

import argparse
import copy
import json
import math
from pathlib import Path
import platform
import re

import routing
import routing_discovery as discovery
import routing_run as runner
from native_io import digest

KIND = 'routing_mac_blocks_v1'
INPUTS = ('manifest', 'discovery_plan', 'discovery_result', 'discovery_review', 'pilot_review')
PACKET = {'kind', 'context', 'qualification', 'witness', 'source', 'cell_id'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def reference(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'sha256': digest(path)}


def checked(bound):
    require(isinstance(bound, dict) and set(bound) == {'path', 'sha256'}
            and routing.sha(bound['sha256']), 'invalid context file binding')
    path = Path(bound['path']).resolve(strict=True)
    require(reference(path) == bound, 'context file identity changed')
    return path


def read(bound, *, sealed=False):
    value = routing.read_json(checked(bound))
    return routing.unseal(value) if sealed else value


def design_valid(design, *, attempts=12):
    require(set(design) == {'attempts', 'warmups', 'seed', 'timeout_s', 'budget_s',
                           'session_id', 'analysis_unit', 'alpha'}, 'freeze every Mac block design field')
    require(type(design['attempts']) is int and design['attempts'] == attempts,
            f'require {attempts} prospectively frozen rounds')
    require(type(design['warmups']) is int and 1 <= design['warmups'] <= 5,
            'freeze 1..5 warmups per distinct arm')
    require(type(design['seed']) is int and 0 <= design['seed'] < 2**64, 'invalid schedule seed')
    require(isinstance(design['session_id'], str) and design['session_id']
            and design['analysis_unit'] == 'block'
            and design['alpha'] == {'numerator': 1, 'denominator': 40}, 'invalid block analysis contract')
    for key, limit in [('timeout_s', 120), ('budget_s', 86400)]:
        require(type(design[key]) in (int, float) and math.isfinite(design[key])
                and 0 < design[key] <= limit, 'invalid prospective ' + key)


def declare(inputs, design):
    """Validate metadata and reviewed evidence before B input access is allowed."""
    require(set(inputs) == set(INPUTS), 'incomplete discovery-to-Mac handoff')
    design_valid(design)
    manifest = read(inputs['manifest'], sealed=True)
    routing.validate_manifest(manifest)
    plan = read(inputs['discovery_plan'], sealed=True)
    summary = read(inputs['discovery_result'], sealed=True)
    review = read(inputs['discovery_review'])
    pilot = read(inputs['pilot_review'])
    if plan.get('kind') == discovery.KIND + '_plan':
        require(plan.get('design') == discovery.DESIGN, 'wrong original SCC discovery design')
        linux_parent = plan
        linux = routing.unseal(plan['tournament'])
        discovery.validate_spec(linux)
    else:
        import routing_parallel
        metadata = routing_parallel.metadata(plan)
        linux_parent, linux = metadata['parent'], metadata['spec']
    require(summary.get('kind') == discovery.KIND + '_shortlist'
            and summary.get('status') == 'discovery_only' and summary.get('promotion') is False
            and summary.get('plan_sha256') == routing.identity(plan), 'invalid discovery-only shortlist')
    shortlist = summary.get('shortlist', [])
    ranking = summary.get('ranking', [])
    require(1 <= len(shortlist) <= 4 and len(set(shortlist)) == len(shortlist)
            and {row['arm'] for row in ranking} == set(linux['candidates'])
            and len(ranking) == len(linux['candidates'])
            and shortlist == [row['arm'] for row in ranking[:4]], 'shortlist differs from complete discovery ranking')
    require(manifest['candidates'] == linux['candidates']
            and manifest['panels'] == linux['panels']
            and manifest['protections'] == linux['protections']
            and manifest['failure_policy'] == linux['failure_policy'], 'Mac handoff changed tournament coverage or policy')
    require(len(manifest['panels']['B']) == 42, 'Mac selection requires all original 42 B cells')
    require(set(manifest['arms']) == set(linux['arms']) and all(
        {k: v for k, v in manifest['arms'][arm].items() if k != 'sha256'} ==
        {k: v for k, v in linux['arms'][arm].items() if k != 'sha256'}
        for arm in manifest['arms']), 'Mac handoff changed an arm policy')
    production = manifest.get('candidate_source_identity', {}).get('production_tokens_sha256')
    require(routing.sha(production) and production == linux.get('candidate_source_identity', {}).get('production_tokens_sha256'),
            'Mac and Linux candidates lack matching production source identity')
    for platform_manifest, parent in [(manifest, None), (linux, linux_parent)]:
        source = platform_manifest.get('candidate_source_identity', {})
        candidate_arms = [arm for arm in platform_manifest['candidates'] if arm != 'baseline']
        hashes = {platform_manifest['arms'][arm]['sha256'] for arm in candidate_arms}
        require(len(hashes) == 1 and all(routing.sha(value) for value in hashes),
                'candidate arms require one platform artifact')
        if parent is None or 'plugin_sha256' in source:
            require(routing.sha(source.get('plugin_sha256')) and hashes == {source['plugin_sha256']},
                    'candidate production source is not bound to its platform artifact')
        else:
            # Older Linux source records bind the artifact through the original
            # frozen parent instead of repeating its hash in the source record.
            # Keep that sealed evidence intact; never read deployment paths here.
            require(routing.sha(source.get('archive_sha256'))
                    and routing.sha(source.get('artifact_inputs_sha256'))
                    and isinstance(source.get('commit'), str)
                    and re.fullmatch(r'[0-9a-f]{40}', source['commit']),
                    'archival Linux source identity is incomplete')
            plugins, frozen = parent.get('plugins'), parent.get('frozen_files')
            require(isinstance(plugins, dict) and isinstance(frozen, dict),
                    'original Linux artifact bindings are missing')
            for arm in candidate_arms:
                path = plugins.get(arm)
                require(isinstance(path, str) and Path(path).is_absolute()
                        and frozen.get(path) == platform_manifest['arms'][arm]['sha256'],
                        'original frozen Linux artifact differs from candidate source')
    expected = {'kind': KIND + '_discovery_review', 'status': 'passed',
                'plan_sha256': routing.identity(plan), 'result_sha256': routing.identity(summary),
                'collection_sha256': summary.get('collection_sha256'),
                'isolation_sha256': summary.get('isolation_sha256'),
                'production_tokens_sha256': production}
    require(all(review.get(key) == value for key, value in expected.items())
            and all(routing.sha(expected[key]) for key in expected if key.endswith('_sha256')),
            'remote aggregate/isolation and production equivalence review missing')
    validation = read(review['remote_validation'])
    require(validation.get('status') == 'passed'
            and validation.get('result_sha256') == routing.identity(summary)
            and validation.get('plan_sha256') == routing.identity(plan), 'actual remote aggregate validation did not pass')
    raw = review.get('raw_evidence', [])
    require(isinstance(raw, list) and raw, 'retain remote validation raw evidence bindings')
    for bound in raw:
        require(checked(bound).stat().st_size > 0, 'empty discovery evidence')
    require(pilot.get('kind') == KIND + '_pilot_review' and pilot.get('status') == 'passed'
            and pilot.get('recommended_attempts') == {'B': 12, 'C': 24}, 'pilot review does not authorize this block design')
    require(pilot.get('evidence'), 'pilot review requires actual evidence')
    for bound in pilot['evidence']:
        checked(bound)
    return {'schema': 1, 'kind': KIND, 'stage': 'B', 'status': 'prospective',
            'inputs': copy.deepcopy(inputs), 'design': copy.deepcopy(design),
            'manifest_sha256': routing.identity(manifest), 'candidates': shortlist,
            'cells': copy.deepcopy(manifest['panels']['B']),
            'protections': list(manifest['protections']),
            'allowances': {'protected': {'relative': .05, 'absolute_seconds': .001},
                           'internal': {'relative': .20, 'absolute_seconds': .001}},
            'coordinator_sha256': digest(Path(__file__)),
            'discovery_validator_sha256': digest(Path(discovery.__file__)),
            'stopping': 'fixed attempts; no replacements; B selects once; C requires public activation'}


def load_context(bound):
    context = read(bound, sealed=True)
    if context.get('kind') == KIND and context.get('stage') == 'B':
        require(context == declare(context['inputs'], context['design']), 'selection context changed')
        return context, read(context['inputs']['manifest'], sealed=True)
    import routing_block_deployment as deployment
    if context.get('kind') == deployment.EQUIVALENCE:
        require(context == deployment.equivalence_context(context['inputs'], context['stage']),
                'activation equivalence context changed')
        return context, context['manifest']
    if context.get('kind') == deployment.KIND:
        require(context == deployment.derive(context['inputs']), 'public C context changed')
        return context, context['manifest']
    raise ValueError('only prospective B or reviewed public activation contexts are executable')


def context_bindings(bound, context):
    if context['kind'] != KIND:
        import routing_block_deployment
        return routing_block_deployment.context_bindings(bound, context)
    values = [bound, *context['inputs'].values(), reference(__file__), reference(discovery.__file__)]
    plan = read(context['inputs']['discovery_plan'], sealed=True)
    if plan.get('kind') != discovery.KIND + '_plan':
        import routing_parallel
        values.append(reference(routing_parallel.__file__))
        if plan.get('legacy_window'):
            import routing_window
            values.append(reference(routing_window.__file__))
    review = read(context['inputs']['discovery_review'])
    values += [review['remote_validation'], *review['raw_evidence']]
    values += read(context['inputs']['pilot_review'])['evidence']
    for value in values:
        checked(value)
    return list({value['path']: value for value in values}.values())


def prerequisite_context(manifest, stage, previous, bound):
    context, original = load_context(bound)
    require(previous is None and stage == context['stage'] and manifest == original,
            'new scientific context cannot use a legacy selection or another stage')
    require(platform.system() == 'Darwin', 'Mac selection requires fresh Mac scientific calls')
    return {'candidates': context['candidates'], 'selection_sha256': routing.identity(context),
            'bindings': context_bindings(bound, context)}


def required_arms(context, cell):
    return list(dict.fromkeys(['baseline', *context['protections'], *context['candidates'], *cell['comparators']]))


def qualification(context_bound, records_bound):
    """Accept only fresh Mac science explicitly generated for this new context."""
    context, manifest = load_context(context_bound)
    records = read(records_bound)
    cells = context['cells']
    arms = set().union(*(set(required_arms(context, cell)) for cell in cells))
    require(records.get('platform', '').startswith('Darwin')
            and records.get('purpose') == 'scientific_prerequisites'
            and records.get('status') in {'complete', 'complete_with_failed_cells'}
            and records.get('stage') == context['stage']
            and records.get('selection_sha256') == routing.identity(context)
            and records.get('qualification_context') == context_bound
            and records.get('manifest_sha256') == routing.identity(manifest)
            and records.get('harness_sha256') == manifest['harness_sha256']
            and records.get('artifacts') == {arm: manifest['arms'][arm]['sha256'] for arm in arms}
            and records.get('cases') == {cell['id']: cell['case_sha256'] for cell in cells}
            and records.get('tool_sha256') == {p.name: h for p, h in runner.tool_identities().items()}
            and records.get('warmups') == 0 and records.get('repetitions') == 1
            and records.get('timeout_s') == manifest['failure_policy']['per_call_timeout_s']
            and records.get('budget_s') == manifest['failure_policy']['aggregate_budget_s'],
            'fresh complete Mac prerequisites for this context are required')
    require(set(records.get('qualified_wrappers', {})) == {cell['id'] for cell in cells},
            'missing Mac scientific wrapper coverage')
    return context, manifest, records


def science_context(packet):
    """Resolve a new timer packet without invoking the old A/B/C selector."""
    require(set(packet) == PACKET and packet['kind'] == 'routing_blocks', 'invalid block science packet')
    context, manifest, records = qualification(packet['context'], packet['qualification'])
    found = [cell for cell in context['cells'] if cell['id'] == packet['cell_id']]
    require(len(found) == 1, 'cell is outside the admitted stage')
    cell = found[0]
    wrapper = records['qualified_wrappers'][cell['id']]
    path = checked(packet['qualification']).parent / wrapper['path']
    require(reference(path) == packet['witness'] and packet['witness']['sha256'] == wrapper['sha256'],
            'witness is not from the fresh Mac qualification')
    require(packet['source']['sha256'] == cell['case_sha256'], 'source differs from frozen cell')
    checked(packet['source'])
    required = required_arms(context, cell)
    _, dependencies = runner.evidence(checked(packet['witness']), cell, manifest, required,
        stage=context['stage'], selection_sha256=routing.identity(context))
    return {'manifest': manifest, 'cell': cell, 'required': required, 'stage': context['stage'],
            'selection_sha256': routing.identity(context),
            'bindings': [*context_bindings(packet['context'], context), packet['qualification'],
                         packet['witness'], packet['source'], *[reference(path) for path in dependencies]]}


def comparison_coverage(context_bound, records_bound):
    """Complete frontier, including unavailable references and identity edges."""
    context, manifest, records = qualification(context_bound, records_bound)
    outcomes = {}
    for cell in context['cells']:
        cid = cell['id']
        required = required_arms(context, cell)
        outcomes[cid] = runner.prerequisite_outcomes(manifest, required,
            [row for row in records['prerequisites'] if row['cell'] == cid],
            [row for row in records['records'] if row['cell'] == cid])
        for arm in [*context['protections'], *cell['comparators']]:
            state = outcomes[cid][arm]['status']
            require(state == 'passed' or (arm not in context['protections'] and state == 'unavailable'),
                    'required or unresolved reference blocks Mac selection')
    candidates = [arm for arm in context['candidates']
                  if all(outcomes[cell['id']][arm]['status'] == 'passed' for cell in context['cells'])]
    require(candidates, 'no scientifically qualified shortlisted candidate')
    comparisons = []
    for cell in context['cells']:
        for candidate in candidates:
            for ref in dict.fromkeys([*context['protections'], *cell['comparators']]):
                comparisons.append({'cell': cell['id'], 'candidate': candidate, 'reference': ref,
                    'reference_role': 'protected' if ref in context['protections'] else 'internal',
                    'status': 'identity' if candidate == ref else
                              'timed' if outcomes[cell['id']][ref]['status'] == 'passed' else 'unavailable'})
    return {'candidates': candidates, 'excluded_candidates': [arm for arm in context['candidates'] if arm not in candidates],
            'comparisons': comparisons, 'outcomes': outcomes}


def timing_spec(config, *, paired=False):
    """Derive complete edges; C requires a separate two-arm block per edge."""
    context, manifest, records = qualification(config['context'], config['qualification'])
    require(context.get('status') != 'activation_equivalence', 'activation equivalence cannot collect timing')
    require((context['stage'] == 'C') == paired, 'C requires paired blocks; B requires mirrored multi-arm blocks')
    coverage = comparison_coverage(config['context'], config['qualification'])
    require(set(config['cells']) == {cell['id'] for cell in context['cells']},
            'complete original ' + context['stage'] + ' cell coverage required')
    require(config['harness']['sha256'] == manifest['harness_sha256'], 'Mac harness changed')
    checked(config['harness'])
    cells = []
    for cell in context['cells']:
        cid = cell['id']
        supplied = config['cells'][cid]
        wrapper = records['qualified_wrappers'][cid]
        packet = {'kind': 'routing_blocks', 'context': config['context'],
                  'qualification': config['qualification'], 'cell_id': cid,
                  'source': supplied['source'],
                  'witness': reference(checked(config['qualification']).parent / wrapper['path'])}
        science_context(packet)
        edges = [row for row in coverage['comparisons'] if row['cell'] == cid and row['status'] == 'timed']
        if not edges:
            continue
        labels = sorted({row[key] for row in edges for key in ['reference', 'candidate']})
        require(set(supplied['arms']) == set(labels), 'timed arm coverage differs from admitted frontier')
        arms = {}
        for arm in labels:
            data = supplied['arms'][arm]
            require(set(data) == {'plugin', 'input'}
                    and data['plugin']['sha256'] == manifest['arms'][arm]['sha256']
                    and data['input']['sha256'] == records['prepared_sha256'][cid + '/' + arm],
                    'timing arm differs from fresh qualified invocation')
            checked(data['plugin']); checked(data['input'])
            arms[arm] = {'label': arm, **data}
        cells.append({'id': cid, 'workload': cid, 'arms': arms, 'validation': packet,
                      'comparisons': [{'id': f'{cid}-p{index:03d}', **{key: row[key] for key in
                          ['reference', 'candidate', 'reference_role']}} for index, row in enumerate(edges)]})
    require(cells, 'no nonidentity runtime comparison is identified')
    if paired:
        cells = [{'id': pair['id'], 'workload': cell['workload'],
                  'arms': {arm: cell['arms'][arm] for arm in [pair['reference'], pair['candidate']]},
                  'validation': cell['validation'], 'comparisons': [pair]}
                 for cell in cells for pair in cell['comparisons']]
    spec = {'schema': 2, 'harness': config['harness'], 'design': context['design'], 'cells': cells,
            'monitor': config['monitor'], 'declarations': [config['context']]}
    import time_blocks
    time_blocks.validate_spec(spec)
    return spec


def decide(config, timing_manifest, records, monitor_receipt):
    """Recompute one conditional B selection; never activate or inspect C."""
    import time_blocks
    context, _ = load_context(config['context'])
    require(context['kind'] == KIND and context['stage'] == 'B', 'B selection cannot consume activation or C evidence')
    expected = timing_spec(config)
    frozen, _ = time_blocks.load_manifest(checked(timing_manifest))
    require(frozen['spec'] == expected, 'timing manifest differs from the complete prospective B design')
    analysis = time_blocks.analyze(checked(timing_manifest), checked(records),
                                  checked(monitor_receipt) if monitor_receipt else None)
    coverage = comparison_coverage(config['context'], config['qualification'])
    edges = {(cell['id'], pair['candidate'], pair['reference']): pair
             for cell in expected['cells'] for pair in cell['comparisons']}
    summaries = {}
    for row in analysis['summary']:
        key = row.get('cell_id'), row.get('candidate_arm'), row.get('reference_arm')
        require(key in edges and key not in summaries and row.get('comparison_id') == edges[key]['id']
                and row.get('reference_role') == edges[key]['reference_role'],
                'analysis contains duplicated, changed or unexpected comparison coverage')
        summaries[key] = row
    valid = not analysis['missing_monitor_evidence'] and not analysis['scientific_or_execution_failure']
    ranking = []
    for candidate in coverage['candidates']:
        comparisons, worst, passed = [], None, valid
        for edge in coverage['comparisons']:
            if edge['candidate'] != candidate:
                continue
            detail = dict(edge)
            if edge['status'] == 'identity':
                detail['pass'] = True
            elif edge['status'] == 'unavailable':
                detail.update({'runtime_identified': False, 'pass': None})
            else:
                summary = summaries.get((edge['cell'], candidate, edge['reference']))
                excess = summary.get('excess_median_interval') if summary else None
                normalized = summary.get('normalized_regret_median_interval') if summary else None
                upper = normalized.get('upper') if normalized else None
                ok = bool(summary and summary['conditional_nonregression_pass']
                    and summary['attempted_slots'] == context['design']['attempts']
                    and excess and excess['upper'] is not None and excess['upper'] <= 0
                    and upper is not None and math.isfinite(upper) and upper <= 1)
                detail.update(runtime_identified=summary is not None, analysis=summary)
                detail['pass'] = ok
                passed &= ok
                if edge['reference_role'] == 'internal' and upper is not None:
                    worst = upper if worst is None else max(worst, upper)
            comparisons.append(detail)
        ranking.append({'arm': candidate, 'pass': bool(passed),
                        'worst_internal_normalized_upper': worst, 'comparisons': comparisons})
    ranking.sort(key=lambda row: (not row['pass'], math.inf if row['worst_internal_normalized_upper'] is None
                                  else row['worst_internal_normalized_upper'], routing.conservative(row['arm'])))
    finalist = next((row['arm'] for row in ranking if row['pass']), None)
    return routing.seal({'schema': 1, 'kind': KIND + '_B_decision',
        'status': 'selected' if finalist else 'no_qualifying_policy', 'finalist': finalist,
        'context': config['context'], 'qualification': config['qualification'],
        'timing_manifest': timing_manifest, 'records': records, 'monitor_receipt': monitor_receipt,
        'analysis_sha256': routing.identity(analysis), 'coordinator_sha256': digest(Path(__file__)),
        'analysis_source_sha256': digest(Path(time_blocks.__file__)), 'ranking': ranking,
        'excluded_candidates': coverage['excluded_candidates'], 'promotion': False,
        'qualification_pass': False, 'requires_public_activation': True,
        'claim': 'Developmental B selection from conditional marginal bounds; no simultaneous or selected-winner coverage.',
        'stopping': 'Freeze one finalist before public activation and C; no C access or runner-up confirmation here.'})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    config = routing.read_json(args.config)
    value = declare(config['inputs'], config['design'])
    with routing.scratch(args.output).open('x') as stream:
        json.dump(routing.seal(value), stream, indent=2, allow_nan=False)
        stream.write('\n')


if __name__ == '__main__':
    main()
