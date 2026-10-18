#!/usr/bin/env python3
"""One prospectively reviewed fresh confirmation after the documented C harness failure.

Frozen selection/activation/science/timing validators remain authoritative. This
module reads metadata and evidence only; it never generates a case, dispatches a
native call, caches a scientific result, or changes the original activation tuple.
Install its two context adapters before installing routing_validation's scoped
metadata guard. All historical contexts delegate to their original validators.
"""
from __future__ import annotations

import argparse
import copy
from contextlib import contextmanager
import json
import math
from pathlib import Path

import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_failure as failure

KIND = 'routing_incremental_confirmation_repair_v1'
MARKER = 'confirmation_protocol'
LIMITS = {'native_timeout_s': 120, 'science_budget_s': 14400, 'timing_budget_s': 28800,
          'oracle_budget_s': 120, 'oracle_outer_s': 125, 'witness_outer_s': 2100,
          'science_outer_s': 15000, 'timing_outer_s': 30000}
STOPPING = ('One fresh confirmation of the existing many-FE finalist. A tooling failure is an engineering '
            'blocker requiring diagnosis and Johannes\'s direction, never automatic C3 or two-FE advance. '
            'Scientific failure or failed/inconclusive performance retains incumbent and requires direction; '
            'no fallback, retuning, minima, observation deletion, retry or continuation.')
CONFIG = {'original_inputs', 'confirmation_failure', 'authorization', 'repair_review', 'history',
          'design', 'limits', 'sources', 'input_seed', 'declared_unix', 'fresh_panel_path',
          'generation_commands', 'python_command'}
SOURCE_KEYS = {'confirmation', 'validation', 'bootstrap', 'witness_adapter', 'supervisor',
               'case_generator', 'panel_generator', 'imports_generator', 'python',
               'python_environment', 'failure_validator'}
CHECKS = {'complete_original_admission', 'all_old_B_wrappers', 'canonical_partial_tuple',
          'synthetic_drift', 'synthetic_cleanup', 'no_fresh_access'}


def require(value, message):
    blocks.require(value, message)


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def same_config(value):
    require(isinstance(value, dict) and set(value) == CONFIG, 'invalid confirmation declaration fields')
    require(set(value['original_inputs']) == deployment.INPUTS, 'preserve original four activation inputs')
    require(value['limits'] == LIMITS and all(type(v) is int for v in value['limits'].values()),
            'confirmation cannot increase or alter frozen work bounds')
    blocks.design_valid(value['design'], attempts=24, incremental=True)
    require(value['design']['warmups'] == 1 and value['design']['timeout_s'] == 120,
            'confirmation keeps one warmup and the original native call cap')
    require(type(value['input_seed']) is int and 0 <= value['input_seed'] < 2**64
            and finite(value['declared_unix']) and value['declared_unix'] > 0,
            'freeze valid fresh input seed and declaration time')
    require(set(value['authorization']) == {'spec', 'workflow'}
            and set(value['history']) == {'many_inputs', 'two_inputs'}
            and isinstance(value['sources'], dict) and SOURCE_KEYS <= set(value['sources']),
            'bind prospective authority, complete history and repaired producer sources')
    require(value['sources']['confirmation'] == blocks.reference(__file__)
            and value['sources']['validation'] == blocks.reference(Path(__file__).with_name('routing_validation.py'))
            and value['sources']['failure_validator'] == blocks.reference(failure.__file__),
            'confirmation or validation utility source changed')
    for bound in [*value['authorization'].values(), *value['history'].values(), *value['sources'].values()]:
        blocks.checked(bound)
    python = Path(value['python_command'])
    require(python.is_absolute() and str(python.resolve(strict=True)) == value['sources']['python']['path']
            and blocks.digest(python) == value['sources']['python']['sha256']
            and value['sources']['python_environment'] == blocks.reference(python.parent.parent / 'pyvenv.cfg'),
            'preserve the reviewed virtual-environment command and canonical interpreter')
    path = Path(value['fresh_panel_path'])
    require(path.is_absolute() and path.name == 'panel.json', 'freeze exact fresh panel metadata location')
    imports = path.parent.parent / 'imports-c'
    panel = [value['python_command'], value['sources']['panel_generator']['path'],
             '--panel', 'C', '--seed', str(value['input_seed']), '--output', str(path.parent),
             '--generator', value['sources']['case_generator']['path'], '--extended', '--sampled',
             '--compact-it', '--low-saved', str(imports / 'low-saved.case')]
    for family in ('no-fe','one-fe','slopes'):
        panel += ['--source', family + '=' + str(imports / (family + '.case'))]
    expected = [{'role':'imports', 'argv':[value['sources']['imports_generator']['path'], str(imports), str(value['input_seed'])]},
                {'role':'panel', 'argv':panel}]
    require(value['generation_commands'] == expected, 'generation must preserve the original imports and panel recipe')
    return value


def proposal_identity(config):
    # The review freezes all action/scientific inputs. Its own binding and the
    # subsequent declaration emission time cannot be part of that prior review.
    return routing.identity({k: v for k, v in config.items() if k not in {'repair_review', 'declared_unix'}})


def partial_tuple_identity(inputs):
    """Canonical return of the actual partial activation used by old A/B science."""
    packet = blocks.read(inputs['activation'])
    equivalent = blocks.read(packet['reports']['equivalence'])
    collection = equivalent['collections'][0]
    partial_context = blocks.read(collection['context'], sealed=True)
    return routing.identity(deployment.activation_core(partial_context['inputs'], equivalence=False))


def history_metadata(config, original):
    histories = {key: blocks.read(bound) for key, bound in config['history'].items()}
    for key, family in [('many_inputs', 'many'), ('two_inputs', 'two')]:
        value = histories[key]
        require(value.get('schema') == 1 and value.get('family') == family
                and set(value.get('panels', {})) == {'A', 'B', 'C'}
                and set(value.get('seeds', {})) == {'A', 'B', 'C'}
                and all(type(seed) is int for seed in value['seeds'].values()),
                'history must retain all many/two panel descriptors and seeds')
        require(all(len(cells) == 42 for cells in value['panels'].values()), 'history needs every 42-cell panel')
    require(histories['many_inputs']['panels'] == original['panels'], 'many history differs from actual original panels')
    generator_hashes = histories['many_inputs'].get('generator_sha256', {})
    require(all(generator_hashes.get(config['sources'][key]['path']) == config['sources'][key]['sha256']
                for key in ('case_generator', 'panel_generator', 'imports_generator')), 'fresh confirmation must use the original generator')
    seeds = {seed for value in histories.values() for seed in value['seeds'].values()}
    return histories, seeds


def repair_evidence(config, original_context, basis):
    review = blocks.read(config['repair_review'])
    require(review.get('kind') == KIND + '_review' and review.get('status') == 'passed'
            and review.get('proposal_sha256') == proposal_identity(config)
            and review.get('failure_basis_sha256') == routing.identity(basis)
            and review.get('sources') == config['sources'], 'repair review is missing, failed or stale')
    report = blocks.read(review['validation_report'])
    require(report.get('kind') == KIND + '_validation' and report.get('status') == 'passed'
            and report.get('original_context_sha256') == routing.identity(original_context)
            and report.get('original_inputs') == config['original_inputs']
            and report.get('sources') == config['sources']
            and set(report.get('checks', {})) == CHECKS
            and all(result is True for result in report['checks'].values())
            and type(report.get('old_B_wrappers')) is int and report['old_B_wrappers'] == 42
            and report.get('canonical_partial_tuple_sha256') == partial_tuple_identity(config['original_inputs'])
            and report.get('repaired_partial_tuple_sha256') == report['canonical_partial_tuple_sha256']
            and finite(report.get('completed_unix')) and finite(review.get('completed_unix'))
            and 0 < report['completed_unix'] <= review['completed_unix'] <= config['declared_unix'],
            'complete old admission, 42 wrappers, tuple, drift/cleanup checks required before fresh generation')
    metadata = report.get('metadata', {})
    require(metadata.get('poisoned') is None and metadata.get('full_byte_checks', 0) >= 2
            and isinstance(metadata.get('closure'), list) and metadata['closure']
            and isinstance(metadata.get('path_aliases'), dict) and metadata['path_aliases'],
            'repair report lacks complete immutable read closure and byte checks')
    for entry in metadata['closure']:
        path = blocks.checked({k: entry[k] for k in ('path', 'sha256')})
        stat = path.stat()
        require(entry.get('stamp') == [stat.st_dev, stat.st_ino, stat.st_mode, stat.st_size,
                                       stat.st_mtime_ns, stat.st_ctime_ns], 'repair validation closure changed')
    for alias, target in metadata['path_aliases'].items():
        require(str(Path(alias).resolve(strict=True)) == target, 'repair validation alias changed')
    for packet in (review, report):
        require(isinstance(packet.get('evidence'), list) and packet['evidence'], 'actual repair evidence missing')
        for bound in packet['evidence']:
            blocks.checked(bound)
    return review, report


def declare(config):
    """Fully rederive the unchanged old admission before fresh data may be generated."""
    same_config(config)
    original, basis = failure.failure_basis(config['confirmation_failure'])
    old = deployment.derive(config['original_inputs'])
    failed = blocks.read(blocks.read(config['confirmation_failure'])['context'], sealed=True)
    require(old == failed and old['inputs'] == config['original_inputs']
            and old.get('experiment_kind') == 'incremental'
            and original['incremental']['family'] == 'many'
            and old['selected'] == 'incremental-many'
            and old['protections'] == ['baseline', 'incumbent', 'selected-hidden'],
            'repair must preserve the actual failed many-FE admission and selected protections')
    histories, seeds = history_metadata(config, original)
    seeds |= {old['design']['seed']}
    old_declaration = blocks.read(config['original_inputs']['declaration'], sealed=True)
    b_context = blocks.read(old_declaration['b_context'], sealed=True)
    seeds.add(b_context['design']['seed'])
    require(config['input_seed'] not in seeds and config['design']['seed'] not in seeds
            and config['input_seed'] != config['design']['seed']
            and config['design']['session_id'] not in {old['design']['session_id'], b_context['design']['session_id']},
            'fresh input and timing seeds/sessions must differ from every historical seed/session')
    require({k:v for k,v in config['design'].items() if k not in ('seed','session_id')} ==
            {k:v for k,v in old['design'].items() if k not in ('seed','session_id')},
            'only confirmation seed and session may change the C design')
    repair_evidence(config, old, basis)
    return {'schema': 1, 'kind': KIND, 'status': 'prospective_after_review_before_fresh_generation',
            'config': copy.deepcopy(config), 'original_context_sha256': routing.identity(old),
            'failure_basis_sha256': routing.identity(basis), 'stopping': STOPPING}


def prior_hashes(original, histories):
    cases, bodies = set(), set()
    def add(cells):
        for cell in cells:
            require(routing.sha(cell.get('case_sha256')) and routing.sha(cell.get('body_sha256')), 'invalid historical hashes')
            cases.add(cell['case_sha256']); bodies.add(cell['body_sha256'])
    def manifest(value):
        for cells in value['panels'].values(): add(cells)
        for prior in value.get('prior_manifests', []): manifest(routing.unseal(prior))
    manifest(original)
    for history in histories.values():
        for cells in history['panels'].values(): add(cells)
    return cases, bodies


def fresh_panel(panel, config, original, histories):
    require(isinstance(panel, dict) and set(panel) == {'schema','panel','seed','cells','compact_it_comparator','stopping_rule'}
            and type(panel['schema']) is int and panel['schema'] == 1 and panel['panel'] == 'C'
            and type(panel['seed']) is int and panel['seed'] == config['input_seed']
            and panel['compact_it_comparator'] is True
            and panel['stopping_rule'] == 'one fixed sparse panel; no Cartesian expansion', 'fresh panel descriptor changed shape')
    cells = panel['cells']; old = original['panels']['C']
    require(isinstance(cells, list) and len(cells) == len(old) == 42, 'fresh confirmation requires all 42 original cell shapes')
    historical_cases, historical_bodies = prior_hashes(original, histories)
    for new, previous in zip(cells, old):
        require(isinstance(new, dict) and {k:v for k,v in new.items() if k not in ('case_sha256','body_sha256')} ==
                {k:v for k,v in previous.items() if k not in ('case_sha256','body_sha256')},
                'fresh panel changed a cell field other than case/body identity')
        require(routing.sha(new.get('case_sha256')) and routing.sha(new.get('body_sha256'))
                and new['case_sha256'] not in historical_cases and new['body_sha256'] not in historical_bodies,
                'fresh confirmation reuses previously declared case or model bytes')
    # Worker variants intentionally share one numeric body; preserve that grouping.
    for i, new in enumerate(cells):
        for j in range(i):
            require((new['body_sha256'] == cells[j]['body_sha256']) ==
                    (old[i]['body_sha256'] == old[j]['body_sha256'])
                    and (new['case_sha256'] == cells[j]['case_sha256']) ==
                    (old[i]['case_sha256'] == old[j]['case_sha256']), 'fresh panel changed worker/input identity grouping')
    return cells


def generation_evidence(bound, declaration_bound, panel_bound, config):
    receipt = blocks.read(bound)
    require(receipt.get('kind') == KIND + '_generation' and receipt.get('status') == 'complete'
            and receipt.get('declaration') == declaration_bound and receipt.get('repair_review') == config['repair_review']
            and receipt.get('panel') == panel_bound and receipt.get('commands') == config['generation_commands']
            and receipt.get('sources') == config['sources'] and receipt.get('seed') == config['input_seed']
            and finite(receipt.get('started_unix')) and finite(receipt.get('completed_unix'))
            and config['declared_unix'] <= receipt['started_unix'] <= receipt['completed_unix']
            and receipt.get('native_calls') == 0 and receipt.get('timed_calls') == 0,
            'fresh generation lacks its actual reviewed declaration, source, command or chronology')
    require(isinstance(receipt.get('evidence'), list) and receipt['evidence'], 'actual generator evidence required')
    for evidence in receipt['evidence']: blocks.checked(evidence)
    return receipt


def derive(declaration_bound, panel_bound, generation_bound):
    declaration = blocks.read(declaration_bound, sealed=True)
    require(declaration.get('kind') == KIND and declaration == declare(declaration['config']), 'repair declaration changed')
    config = declaration['config']
    require(panel_bound['path'] == config['fresh_panel_path'], 'fresh panel moved outside its prospectively frozen path')
    panel = blocks.read(panel_bound)
    generation_evidence(generation_bound, declaration_bound, panel_bound, config)
    original, _ = failure.failure_basis(config['confirmation_failure'])
    histories, _ = history_metadata(config, original)
    cells = fresh_panel(panel, config, original, histories)
    context = copy.deepcopy(deployment.derive(config['original_inputs']))
    context['manifest']['panels']['C'] = copy.deepcopy(cells)
    context.update(cells=copy.deepcopy(cells), design=copy.deepcopy(config['design']), stopping=STOPPING)
    context[MARKER] = {'version': KIND, 'declaration': declaration_bound,
                       'panel': panel_bound, 'generation': generation_bound}
    return context


def marker(context):
    value = context.get(MARKER)
    require(isinstance(value, dict) and set(value) == {'version','declaration','panel','generation'}
            and value['version'] == KIND and context.get('kind') == deployment.KIND
            and context.get('stage') == 'C', 'malformed or unsupported confirmation version marker')
    return value


def failure_dependencies(bound):
    """Freeze every metadata/raw dependency used by the unchanged failure validator."""
    packet = blocks.read(bound)
    values = {key: blocks.read(packet[key], sealed=key in {'context', 'handoff'})
              for key in failure.REFERENCES}
    start, admitted, admission, handoff, review = (values[key] for key in
        ('start', 'admission_outcome', 'admission_review', 'handoff', 'raw_review'))
    refs = [bound, *[packet[key] for key in failure.REFERENCES], *packet['partial_outputs']]
    refs += [start[key] for key in ('supervisor', 'owned_runner', 'witness_adapter', 'source_review')]
    refs += [admission['supervisor_outcome'], *admitted['sources'].values(),
             handoff['config_snapshot'], *handoff['closure']]
    refs += [{k: entry[k] for k in ('path', 'sha256')} for entry in admitted['metadata']['closure']]
    refs += [review[key] for key in ('audit_source', 'closure_capture', 'lead_failure_validation',
                                    'supervisor_log', 'witness_log')]
    for ref in refs: blocks.checked(ref)
    return refs


def dependencies(context):
    value = marker(context); declaration = blocks.read(value['declaration'], sealed=True); config = declaration['config']
    review = blocks.read(config['repair_review']); report = blocks.read(review['validation_report'])
    receipt = blocks.read(value['generation'])
    refs = [*value.values()]
    refs = [ref for ref in refs if isinstance(ref, dict)]
    refs += [config['confirmation_failure'], config['repair_review'], *config['authorization'].values(),
             *config['history'].values(), *config['sources'].values(), review['validation_report'],
             *review['evidence'], *report['evidence'], *receipt['evidence']]
    refs += [{k: entry[k] for k in ('path','sha256')} for entry in report['metadata']['closure']]
    refs += failure_dependencies(config['confirmation_failure'])
    for ref in refs: blocks.checked(ref)
    return refs


@contextmanager
def install():
    """Only the exact version marker dispatches; historical inputs remain unchanged."""
    prior_load, prior_bindings = blocks.load_context, blocks.context_bindings
    def load(bound):
        context = blocks.read(bound, sealed=True)
        if MARKER not in context: return prior_load(bound)
        value = marker(context)
        expected = derive(value['declaration'], value['panel'], value['generation'])
        require(context == expected, 'versioned confirmation context changed')
        return context, context['manifest']
    def bindings(bound, context):
        if MARKER not in context: return prior_bindings(bound, context)
        value = marker(context)
        require(context == derive(value['declaration'], value['panel'], value['generation']), 'versioned binding context changed')
        refs = prior_bindings(bound, context) + dependencies(context)
        return list({ref['path']:ref for ref in refs}.values())
    blocks.load_context, blocks.context_bindings = load, bindings
    try: yield
    finally: blocks.load_context, blocks.context_bindings = prior_load, prior_bindings


def terminal_history(context):
    value = marker(context)
    declaration = blocks.read(value['declaration'], sealed=True)
    return {'version': KIND, 'context_sha256': routing.identity(context),
            'declaration_sha256': routing.identity(declaration), 'panel': routing.seal(blocks.read(value['panel'])),
            'generation_sha256': value['generation']['sha256'],
            'original_context_sha256': declaration['original_context_sha256']}


def terminal_stage(inputs):
    """Standard terminal semantics plus a reviewed record of the newly exposed C."""
    with install():
        result = deployment.terminal_stage(inputs)
        config = blocks.read(inputs['config'])
        context, _ = blocks.load_context(config['context'])
        require(result['export_review']['report'].get('confirmation_history') == terminal_history(context),
                'terminal export must preserve the fresh confirmation history')
    return result


def validate_terminal(value, *, portable=False):
    with install(): deployment.validate_terminal(value, portable=portable)
    history = value['export_review']['report'].get('confirmation_history')
    require(isinstance(history, dict) and set(history) == {'version','context_sha256','declaration_sha256',
            'panel','generation_sha256','original_context_sha256'} and history['version'] == KIND
            and all(routing.sha(history[k]) for k in ['context_sha256','declaration_sha256',
                'generation_sha256','original_context_sha256']), 'missing versioned terminal freshness history')
    panel = routing.unseal(history['panel'])
    original = routing.unseal(value['prior_manifest'])
    # Portable validation preserves old manifest truth and exposes additional C
    # identities. Later stages must explicitly add these hashes to their history.
    fresh_panel(panel, {'input_seed': panel['seed']}, original, {})
    if not portable:
        require(value == terminal_stage(value['inputs']), 'versioned terminal differs from actual fresh C')
    return value


def additional_history(value):
    """Explicit freshness extension for any later stage; never rewrite the old manifest."""
    validate_terminal(value, portable=True)
    cells = routing.unseal(value['export_review']['report']['confirmation_history']['panel'])['cells']
    return {'cases': sorted({c['case_sha256'] for c in cells}), 'bodies': sorted({c['body_sha256'] for c in cells})}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    a = sub.add_parser('declare'); a.add_argument('--config', type=Path, required=True)
    a = sub.add_parser('derive')
    for key in ('declaration','panel','generation'): a.add_argument('--'+key, type=Path, required=True)
    for a in sub.choices.values(): a.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = declare(blocks.read(blocks.reference(args.config))) if args.command == 'declare' else derive(
        *[blocks.reference(getattr(args,key)) for key in ('declaration','panel','generation')])
    with routing.scratch(args.output).open('x') as stream:
        json.dump(routing.seal(result), stream, indent=2, sort_keys=True, allow_nan=False); stream.write('\n')


if __name__ == '__main__': main()
