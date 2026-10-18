#!/usr/bin/env python3
"""Retain an incumbent after a bounded C witness failure before native science.

This separate extension leaves frozen selector/confirmation sources unchanged.
Prospective callers explicitly use install() for local terminal replay. Portable
validation remains an attestation check, never a substitute for this raw replay.
There is no acceptance, producer retry, synthetic timing decision or B reanalysis.
"""
from __future__ import annotations

import argparse
import copy
from contextlib import contextmanager
import json
from pathlib import Path

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment

KIND = 'routing_incremental_confirmation_failure'
INPUTS = {'confirmation_failure', 'operational_review'}
REFERENCES = ('context', 'start', 'outcome', 'handoff', 'admission_outcome',
              'admission_review', 'raw_review')


def failure_basis(bound):
    packet = blocks.read(bound)
    blocks.require(set(packet) == {'schema', 'kind', *REFERENCES, 'partial_outputs'}
                   and packet['schema'] == 1 and packet['kind'] == KIND,
                   'invalid pre-science failure packet')
    values = {key: blocks.read(packet[key], sealed=key in {'context', 'handoff'})
              for key in REFERENCES}
    context, start, outcome, handoff, admitted, admission, review = (
        values[key] for key in REFERENCES)
    blocks.require(context.get('kind') == deployment.KIND and context.get('stage') == 'C'
        and context.get('experiment_kind') == 'incremental', 'failure requires admitted incremental C')
    declaration = blocks.read(context['inputs']['declaration'], sealed=True)
    b_context = blocks.read(declaration['b_context'], sealed=True)
    original = blocks.read(b_context['inputs']['manifest'], sealed=True)
    routing.validate_manifest(original)
    blocks.require(context['manifest'] == deployment.holdout_manifest(
        original, context['selected'], context['manifest']['arms'][deployment.PUBLIC])
        and context['cells'] == original['panels']['C'] and context['design'] == declaration['design'],
        'failed C differs from its declared family, cells or design')
    for ref in context['inputs'].values():
        blocks.checked(ref)
    blocks.require(start.get('status') == 'started' and start.get('results') == []
        and start.get('phase') == 'C' and start.get('context') == packet['context']
        and outcome.get('status') == 'terminal' and outcome.get('exit_code') == 1,
        'failure lacks matching started/failed producer')
    for key in ('pid', 'started_unix', 'phase', 'context', 'commands', 'supervisor',
                'owned_runner', 'witness_adapter', 'source_review', 'native_timeout_s', 'native_budget_s'):
        blocks.require(start[key] == outcome[key], 'producer identity or frozen limits changed')
    for key in ('supervisor', 'owned_runner', 'witness_adapter', 'source_review'):
        blocks.checked(start[key])
    commands = start['commands']
    blocks.require([c['name'] for c in commands] == ['witnesses', 'science']
        and start['native_timeout_s'] == 120 and start['native_budget_s'] == 14400
        and commands[0]['outer_seconds'] == 2100 and commands[1]['outer_seconds'] == 15000,
        'failure is outside the bounded witness contract')
    results = outcome['results']
    blocks.require(len(results) == 1 and results[0]['name'] == 'witnesses'
        and outcome.get('error', '').startswith("TimeoutError('runner exceeded 2100s;")
        and 2100 <= outcome['elapsed_seconds'] <= 2120
        and abs(outcome['completed_unix'] - outcome['started_unix'] - outcome['elapsed_seconds']) < .01,
        'failure must be the actual bounded pre-science timeout')
    ownership = results[0]['ownership']
    blocks.require(ownership['identities'] and ownership['cleanup'].get('complete') is True
        and ownership['cleanup'].get('survivors') == [], 'failed producer cleanup incomplete')
    blocks.require(admitted.get('status') == 'passed' and admitted.get('action') == 'derive-c'
        and admitted.get('outputs') == {'context': packet['context']}
        and admitted['completed_unix'] < start['started_unix']
        and admission.get('status') == 'passed'
        and admission.get('kind') == 'independent_C_admission_review'
        and admission.get('context') == packet['context']
        and admission.get('complete_activation') == context['inputs']['activation']
        and admission.get('declaration') == context['inputs']['declaration']
        and admission.get('metadata_outcome') == packet['admission_outcome'],
        'failure lacks actual independently reviewed C admission')
    supervisor = blocks.read(admission['supervisor_outcome'])
    blocks.require(supervisor.get('status') == 'passed' and supervisor.get('action') == 'derive-c'
        and supervisor.get('metadata_outcome') == packet['admission_outcome']
        and supervisor.get('outputs') == admitted['outputs']
        and admitted['metadata'].get('poisoned') is None, 'C admission postflight failed')
    metadata = admitted['metadata']
    blocks.require(metadata.get('closure') and metadata.get('full_byte_checks', 0) >= 2,
                   'actual metadata read closure and byte checks required')
    for entry in metadata['closure']:
        path = blocks.checked({k: entry[k] for k in ('path', 'sha256')})
        stat = path.stat()
        blocks.require(entry['stamp'] == [stat.st_dev, stat.st_ino, stat.st_mode,
            stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns], 'admission file identity changed')
    blocks.require(metadata.get('path_aliases'), 'admission aliases required')
    for alias, target in metadata['path_aliases'].items():
        blocks.require(str(Path(alias).resolve(strict=True)) == target, 'admission alias changed')
    for ref in admitted['sources'].values():
        blocks.checked(ref)
    blocks.require(handoff.get('phase') == 'C' and handoff.get('context') == packet['context']
        and handoff.get('context_identity') == routing.identity(context)
        and handoff.get('manifest_identity') == routing.identity(context['manifest'])
        and handoff.get('adapter') == start['witness_adapter'] and handoff.get('closure'),
        'failed oracle handoff changed')
    for ref in handoff['closure']:
        blocks.checked(ref)
    snapshot = blocks.read(handoff['config_snapshot'])
    blocks.require(snapshot['activation_complete'] == context['inputs']['activation'],
                   'oracle snapshot used another activation')
    area = blocks.checked(packet['context']).parent
    absent = [area / name for name in ('C-science.log', 'C/science', 'C/timing-manifest.json',
              'C/timing', 'C/decision.json', 'C-freeze-timing-supervisor-start.json')]
    blocks.require(all(not p.exists() for p in absent), 'native science or timing already started')
    partial = packet['partial_outputs']
    actual_paths = {str(p.resolve()) for p in (area / 'C/ready').rglob('*') if p.is_file()}
    blocks.require(partial and len(partial) == len(actual_paths)
        and {ref['path'] for ref in partial} == actual_paths, 'partial witness inventory incomplete')
    for ref in partial:
        blocks.checked(ref)
    blocks.require(review.get('kind') == KIND + '_review' and review.get('status') == 'passed_failure_review'
        and all(review.get(key) == packet[key] for key in REFERENCES if key != 'raw_review')
        and review.get('partial_outputs') == partial
        and all(review.get(key) is True for key in ('no_native_science', 'no_timing', 'cleanup_complete')),
        'independent raw failure review differs from actual operation')
    for ref in [review[key] for key in ('audit_source', 'closure_capture', 'lead_failure_validation',
                                      'supervisor_log', 'witness_log')]:
        blocks.checked(ref)
    cleanup = blocks.read(review['lead_failure_validation'])
    blocks.require(cleanup.get('status') == 'passed_failure_validation'
        and cleanup.get('outcome') == packet['outcome'] and cleanup.get('owned_cleanup_complete') is True
        and cleanup.get('current_owned_census')
        and all(row['ps_exit'] == 1 and row['ps'] == '' for row in cleanup['current_owned_census']),
        'independent cleanup census is incomplete')
    census = {str(row['pid']): row for row in cleanup['current_owned_census']}
    blocks.require(len(census) == len(cleanup['current_owned_census'])
        and set(census) == {str(start['pid']), *ownership['identities']}
        and all(census[pid]['birth'].strip() == birth.strip()
                for pid, birth in ownership['identities'].items()), 'owned process census changed identity')
    basis = {'schema': 1, 'kind': KIND + '_basis', 'status': 'not_confirmed',
        'evidence_kind': 'bounded_witness_timeout_before_native_science',
        'confirmation_failure': bound, 'selected': context['selected'],
        'promotion': False, 'qualification_pass': False, 'native_calls': 0, 'timed_calls': 0,
        'stopping': 'retain incumbent; no retry, replacement, post-C selection or retuning'}
    return original, basis


def terminal_stage(inputs):
    """Produce only a retained terminal, with the existing platform/export contract."""
    blocks.require(set(inputs) == INPUTS, 'wrong failed-confirmation terminal inputs')
    original, basis = failure_basis(inputs['confirmation_failure'])
    review = blocks.read(inputs['operational_review'])
    blocks.require(review.get('kind') == deployment.TERMINAL + '_operational_review'
        and review.get('status') == 'passed' and review.get('outcome') == 'retained'
        and review.get('source_decision_sha256') == routing.identity(basis)
        and review.get('evidence_kind') == basis['evidence_kind']
        and review.get('failure_validator') == blocks.reference(__file__),
        'failure terminal can only retain the incumbent under reviewed failure semantics')
    source = blocks.read(review['incumbent_source'])
    revision, artifacts = source['revision'], source['artifacts']
    identity = {k: source[k] for k in ('production_tokens_sha256', 'artifact_inputs_sha256')}
    blocks.require(revision == original['incremental']['incumbent_revision']
        and set(artifacts) == {'Darwin', 'Linux'}
        and artifacts['Darwin'] == original['arms']['incumbent']['sha256']
        and all(routing.sha(v) for v in [*identity.values(), *artifacts.values()]),
        'retained source or platform artifacts changed')
    builds = review['platform_builds']
    blocks.require(set(builds) == {'Darwin', 'Linux'}, 'both actual platform builds required')
    for host, ref in builds.items():
        build = blocks.read(ref)
        blocks.require(build.get('status') == 'passed' and build.get('platform') == host
            and build.get('revision') == revision and build.get('plugin_sha256') == artifacts[host]
            and all(build.get(k) == v for k, v in identity.items()), 'platform source/build mismatch')
        blocks.checked(build['evidence'])
    blocks.require(review.get('evidence'), 'operational evidence required')
    for ref in review['evidence']:
        blocks.checked(ref)
    exported_ref = review['independent_export_review']
    exported = blocks.read(exported_ref)
    value = {'schema': 1, 'kind': deployment.TERMINAL, 'stage_family': original['incremental']['family'],
        'status': 'retained', 'source_decision_sha256': routing.identity(basis),
        'incumbent_revision': revision, 'incumbent_artifacts': artifacts,
        'incumbent_source_identity': identity, 'incumbent_source_sha256': review['incumbent_source']['sha256'],
        'export_review': {'binding': exported_ref, 'report': exported,
                          'text': blocks.checked(exported_ref).read_text()},
        'prior_manifest': routing.seal(original), 'inputs': copy.deepcopy(inputs)}
    deployment.validate_terminal(value, portable=True)
    blocks.require(exported.get('evidence_kind') == basis['evidence_kind']
        and exported.get('failure_validator') == review['failure_validator'],
        'independent export must identify the failure validator and non-timing basis')
    for ref in exported['evidence']:
        blocks.checked(ref)
    return value


@contextmanager
def install():
    """Explicit scoped dispatch for prospective callers; old protocols are untouched."""
    previous = deployment.terminal_stage

    def dispatch(inputs):
        return terminal_stage(inputs) if set(inputs) == INPUTS else previous(inputs)

    deployment.terminal_stage = dispatch
    try:
        yield
    finally:
        deployment.terminal_stage = previous


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = terminal_stage(json.loads(args.inputs.read_text()))
    with routing.scratch(args.output).open('x') as stream:
        json.dump(routing.seal(result), stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write('\n')


if __name__ == '__main__':
    main()
