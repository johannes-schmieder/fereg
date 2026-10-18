#!/usr/bin/env python3
"""Once-only supervised operations for the reviewed many-FE confirmation repair.

This driver never generates a panel, changes a validator, caches science, retries
an operation, or activates a default. Review/configuration files are inputs, not
produced approvals. ``exposed`` reads only previously exposed evidence. Every
other action consumes the preceding successful child AND supervisor receipts.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
for _name in ('PYTHONDONTWRITEBYTECODE', 'OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS',
              'MKL_NUM_THREADS', 'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS'):
    if os.environ.get(_name) != '1':
        raise RuntimeError('confirmation driver requires ' + _name + '=1 before import')
import block_host
import routing
import routing_blocks as blocks
import routing_block_deployment as deployment
import routing_confirmation as confirmation
import routing_oracle
import routing_run as runner
from routing_validation import MetadataValidation, ByteVerificationPass
import time_blocks

KIND = confirmation.KIND
OWNED_SHA = '2af41e4d933be8e6e76b6d737e45edc01800c27263e40b1d9b9d7dd53bd42ad4'
ACTIONS = ('derive', 'witnesses', 'science', 'science-postflight', 'freeze-timing', 'collect', 'decide')
BOUNDS = {'exposed': 1800, 'derive': 1800, 'witnesses': 2100, 'science': 15000,
          'science-postflight': 1800, 'freeze-timing': 1800, 'collect': 30000, 'decide': 1800}
COMMON = {'schema', 'kind', 'output_root', 'sources', 'python_command'}
EXPOSED = COMMON | {'original_inputs', 'original_context', 'old_b_context', 'old_b_sources',
                    'old_b_wrappers', 'baseline_profile', 'synthetic_checks'}
EXECUTION = COMMON | {'declaration', 'panel', 'generation', 'harness', 'plugins',
                     'resource_contract', 'source_checks', 'host_policy'}
LIMITATION = ('Successful native raw arrays are deleted by the unchanged runner. This driver '
             'retains its original independent proof/fingerprint checks and does not claim a '
             'second exhaustive independent audit of deleted arrays. Timing is never intercepted.')


def require(value, message):
    if not value:
        raise ValueError(message)


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('x') as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def ref(path):
    return blocks.reference(path)


def read(bound, verify=None, *, sealed=False):
    path = verify.check(bound) if verify else blocks.checked(bound)
    value = routing.read_json(path)
    return routing.unseal(value) if sealed else value


def bind_sources(config, verify):
    sources = config['sources']
    require(confirmation.SOURCE_KEYS | {'owned_runner', 'driver_tests'} <= set(sources),
            'bind all actual producers, owned process helper and focused tests')
    expected = {'confirmation': confirmation.__file__, 'validation': Path(__file__).with_name('routing_validation.py'),
                'failure_validator': confirmation.failure.__file__, 'bootstrap': __file__,
                'witness_adapter': __file__, 'supervisor': __file__, 'python': sys.executable,
                'driver_tests': Path(__file__).parent / 'tests/test_routing_confirmation_run.py'}
    for key, path in expected.items():
        require(sources[key]['path'] == str(Path(path).resolve(strict=True)), 'wrong actual source: ' + key)
    require(sources['owned_runner']['sha256'] == OWNED_SHA, 'unsupported process ownership helper')
    for bound in sources.values():
        verify.check(bound)
    command = Path(config['python_command'])
    require(command.is_absolute() and str(Path(sys.executable).absolute()) == str(command)
            and command.resolve(strict=True) == Path(sources['python']['path']), 'preserve designated Python invocation alias')
    verify.digest(command)
    require(sources.get('python_environment') == ref(command.parent.parent / 'pyvenv.cfg'), 'bind actual virtual environment')


def configuration(bound, review_bound, action, verify):
    config = read(bound, verify)
    exposed = config.get('kind') == KIND + '_exposed_run'
    require(config.get('schema') == 1 and set(config) == (EXPOSED if exposed else EXECUTION)
            and config['kind'] == KIND + ('_exposed_run' if exposed else '_execution_run'), 'invalid run configuration')
    require((action == 'exposed') == exposed and action in BOUNDS, 'action/configuration mismatch')
    output = Path(config['output_root'])
    require(output.is_absolute() and output.resolve() == output
            and output.is_relative_to(ROOT / 'target/tmp/auto-routing'), 'output must remain in task directory')
    bind_sources(config, verify)
    review = read(review_bound, verify)
    require(review.get('kind') == KIND + '_driver_review' and review.get('status') == 'passed'
            and review.get('config') == bound and review.get('sources') == config['sources']
            and review.get('allowed_actions') == (['exposed'] if exposed else list(ACTIONS))
            and type(review.get('completed_unix')) in (int, float)
            and 0 < review['completed_unix'] <= time.time(), 'missing or stale independent driver review')
    for evidence in review.get('evidence', []):
        verify.check(evidence)
    return config


def owned(config):
    path = blocks.checked(config['sources']['owned_runner'])
    require(config['sources']['owned_runner']['sha256'] == OWNED_SHA, 'unsupported owned helper')
    spec = importlib.util.spec_from_file_location('_confirmation_owned_helper', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # Its old main is never invoked.
    return module


@contextmanager
def immutable_dependencies(verify):
    """Only the existing frozen_dependencies callback; proof/native hashes stay fresh."""
    original = runner.evidence
    def evidence(*args, **kwargs):
        if kwargs.get('dependency_verifier') is None:
            kwargs['dependency_verifier'] = verify.digest
        result = original(*args, **kwargs)
        for path, digest in result[1].items():
            verify.check({'path': str(Path(path).resolve(strict=True)), 'sha256': digest})
        return result
    runner.evidence = evidence
    try:
        yield
    finally:
        runner.evidence = original


def tracked_evidence(path, cell, manifest, arms, verify, **kwargs):
    value, dependencies = runner.evidence(path, cell, manifest, arms,
                                         dependency_verifier=verify.digest, **kwargs)
    for path, digest in dependencies.items():
        verify.check({'path': str(Path(path).resolve(strict=True)), 'sha256': digest})
    return value


def source_case(root, cell, verify):
    source = Path(root) / cell['case_file']
    require(source.resolve(strict=True).is_relative_to(Path(root).resolve(strict=True)), 'case escapes source directory')
    verify.check({'path': str(source.resolve(strict=True)), 'sha256': cell['case_sha256']})
    verify.digest(source.with_suffix('.json'))
    meta = routing.read_json(source.with_suffix('.json'))
    local = 'oracle' in meta or 'replay_reference' in meta or cell['family'] == 'inactive-columns'
    return source, local


def exposed(config, verify, review_bound):
    declaration = read(config['original_inputs']['declaration'], verify, sealed=True)
    require(config['old_b_context'] == declaration['b_context'], 'exposed B must be the original admitted B context')
    original = deployment.derive(config['original_inputs'])
    require(original == read(config['original_context'], verify, sealed=True), 'original C admission changed')
    for bound in blocks.context_bindings(config['original_context'], original):
        verify.check(bound)
    canonical = read(config['baseline_profile'], verify)
    require(canonical.get('status') == 'passed', 'original profile did not pass')
    activation = read(config['original_inputs']['activation'], verify)
    equivalent = read(activation['reports']['equivalence'], verify)
    partial_context = read(equivalent['collections'][0]['context'], verify, sealed=True)
    require(canonical.get('inputs') == partial_context['inputs'], 'baseline profile did not measure the actual partial activation')
    partial = confirmation.partial_tuple_identity(config['original_inputs'])
    require(partial == canonical['return_sha256'], 'canonical partial activation return changed')
    context, manifest = blocks.load_context(config['old_b_context'])
    require(context['stage'] == 'B' and len(context['cells']) == 42, 'exact original B panel required')
    for bound in blocks.context_bindings(config['old_b_context'], context):
        verify.check(bound)
    for cell in context['cells']:
        _, local = source_case(config['old_b_sources'], cell, verify)
        tracked_evidence(Path(config['old_b_wrappers']) / (cell['id'] + '.json'), cell, manifest,
                         blocks.required_arms(context, cell), verify, prerequisites_only=True, local_oracle=local)
    checks = read(config['synthetic_checks'], verify)
    require(checks.get('status') == 'passed' and checks.get('checks', {}).get('synthetic_drift') is True
            and checks.get('checks', {}).get('synthetic_cleanup') is True
            and checks.get('sources') == config['sources'] and checks.get('evidence'), 'bound actual synthetic checks required')
    for bound in checks['evidence']:
        verify.check(bound)
    verify.before_producer()
    return {'kind': KIND + '_validation', 'status': 'passed', 'original_inputs': config['original_inputs'],
            'original_context_sha256': routing.identity(original), 'sources': config['sources'],
            'canonical_partial_tuple_sha256': canonical['return_sha256'],
            'repaired_partial_tuple_sha256': partial, 'old_B_wrappers': 42,
            'checks': {key: True for key in confirmation.CHECKS},
            'evidence': [config['baseline_profile'], config['synthetic_checks'], review_bound]}


def prior(config, config_bound, review_bound, action, verify):
    if action in ('exposed', 'derive'):
        return
    predecessor = ACTIONS[ACTIONS.index(action) - 1]
    root = Path(config['output_root'])
    child_bound = ref(root / predecessor / 'outcome.json')
    child = read(child_bound, verify)
    supervisor = read(ref(root / predecessor / 'supervisor-outcome.json'), verify)
    for value in (child, supervisor):
        require(value.get('status') == 'passed' and value.get('action') == predecessor
                and value.get('config') == config_bound and value.get('review') == review_bound
                and value.get('sources') == config['sources'], 'predecessor failed or changed')
    require(supervisor.get('child_outcome') == child_bound, 'supervisor did not attest this child outcome')
    for bound in child['outputs'].values():
        verify.check(bound)
    for bound in supervisor.get('outputs', {}).values():
        verify.check(bound)


def cohort(config, verify):
    bound = ref(Path(config['output_root']) / 'derive/context.json')
    verify.check(bound)
    context, manifest = blocks.load_context(bound)
    for dependency in blocks.context_bindings(bound, context):
        verify.check(dependency)
    require(context['stage'] == 'C' and len(context['cells']) == 42
            and context['design']['attempts'] == 24 and context['design']['budget_s'] == 28800
            and context['design']['timeout_s'] == 120, 'frozen C bounds changed')
    return bound, context, manifest


def products(config, manifest, verify):
    available = {bound['sha256']: verify.check(bound) for bound in config['plugins'].values()}
    return {arm: available[value['sha256']] for arm, value in manifest['arms'].items()}


def oracle_command(config_bound, review_bound, handoff_bound, source_bound, output):
    return [sys.executable, str(Path(__file__).resolve()), 'oracle-worker',
            '--config', json.dumps(config_bound), '--review', json.dumps(review_bound),
            '--handoff', json.dumps(handoff_bound), '--source', json.dumps(source_bound), '--output', str(output)]


def witness_worker(handoff_bound, source_bound, output, config_bound, review_bound):
    """Parent owns full admission closure; worker checks its exact immutable handoff."""
    verify, result, outcome = ByteVerificationPass(), None, None
    try:
        with verify:
            handoff = read(handoff_bound, verify, sealed=True)
            config = configuration(config_bound, review_bound, 'witnesses', verify)
            require(handoff['config'] == config_bound and handoff['review'] == review_bound
                    and handoff['sources'] == config['sources'], 'oracle handoff changed')
            lock = ROOT / 'target/tmp/auto-routing/many/confirmation-repair-execution.json'
            require(read(ref(lock), verify) == {'config': config_bound, 'review': review_bound,
                    'declaration': config['declaration']}, 'oracle outside sole reviewed confirmation version')
            require(handoff['context'] == ref(Path(config['output_root']) / 'derive/context.json')
                    and Path(handoff['ready']) == Path(config['output_root']) / 'witnesses/ready',
                    'oracle handoff outside current execution')
            context = read(handoff['context'], verify, sealed=True)
            cell = next(c for c in context['cells'] if c['case_sha256'] == source_bound['sha256'])
            source, _ = source_case(Path(source_bound['path']).parent, cell, verify)
            require(ref(source) == source_bound and Path(output) == Path(handoff['ready']) / (source.stem + '-oracle.json'),
                    'oracle source/output mismatch')
            parent = read(ref(Path(output).with_name(Path(output).stem + '-start.json')), verify)
            require(parent.get('kind') == KIND + '_oracle_supervisor_start' and parent.get('status') == 'started'
                    and all(parent.get(key) == bound for key, bound in
                            [('source', source_bound), ('handoff', handoff_bound), ('config', config_bound), ('review', review_bound)])
                    and parent.get('command') == oracle_command(config_bound, review_bound, handoff_bound, source_bound, output),
                    'oracle lacks exact parent start/source/command')
            start = {'kind': KIND + '_oracle_start', 'source': source_bound, 'handoff': handoff_bound,
                     'config': config_bound, 'review': review_bound, 'sources': config['sources'], 'started_unix': time.time()}
            save(Path(output).with_suffix('.worker-start.json'), start)
            outcome = Path(output).with_suffix('.worker-outcome.json')
            result = {**start, 'kind': KIND + '_oracle_outcome', 'status': 'failed'}
            verify.before_producer()
            old = signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError('oracle 120s budget')))
            signal.alarm(120)
            try:
                routing_oracle.write_reference(source, Path(output))
                routing_oracle.load_reference(Path(output), cell['case_sha256'], cell['body_sha256'])
            finally:
                signal.alarm(0)
                signal.signal(signal.SIGALRM, old)
        result.update(status='passed', output=ref(output))
    except BaseException as error:
        if result is not None:
            result['error'] = repr(error)
        raise
    finally:
        if result is not None:
            result.update(completed_unix=time.time(), metadata=verify.report())
            save(outcome, result)


def witnesses(config, config_bound, review_bound, verify, area):
    context_bound, context, manifest = cohort(config, verify)
    ready = area / 'ready'
    ready.mkdir(exist_ok=False)
    dependencies = [config_bound, review_bound, config['resource_contract'], *config['source_checks'],
                    *config['sources'].values(), *blocks.context_bindings(context_bound, context)]
    for bound in dependencies:
        verify.check(bound)
    # The sealed handoff records the complete admitted closure; workers do not
    # repeat the many-gigabyte B admission within their independent math budget.
    handoff = {'config': config_bound, 'review': review_bound, 'sources': config['sources'],
               'context': context_bound, 'ready': str(ready), 'metadata': verify.report()}
    save(area / 'handoff.json', routing.seal(handoff))
    handoff_bound = ref(area / 'handoff.json')
    verify.check(handoff_bound)
    helper = owned(config)
    oracle_receipts = []
    for cell in context['cells']:
        source, local = source_case(Path(config['panel']['path']).parent, cell, verify)
        arms = blocks.required_arms(context, cell)
        wrapper = {'schema': 2, 'status': 'ready', 'manifest_sha256': routing.identity(manifest),
                   **{key: cell[key] for key in ('case_sha256', 'body_sha256', 'threads')},
                   'cell_id': cell['id'], 'arms': {arm: manifest['arms'][arm] for arm in arms},
                   'resource_contract': config['resource_contract'], 'frozen_dependencies': dependencies + [handoff_bound]}
        if not local:
            output = ready / (source.stem + '-oracle.json')
            if not output.exists():
                entry = {'source': ref(source), 'started_unix': time.time(), 'status': 'failed',
                         'config': config_bound, 'review': review_bound, 'handoff': handoff_bound}
                command = oracle_command(config_bound, review_bound, handoff_bound, ref(source), output)
                save(ready / (source.stem + '-oracle-start.json'), {**entry, 'command': command,
                     'kind': KIND + '_oracle_supervisor_start', 'status': 'started'})
                try:
                    verify.before_producer()
                    helper.run_owned(command, ready / (source.stem + '-oracle.log'), 125, entry)
                    verify.check_bytes()
                    worker_bound = ref(output.with_suffix('.worker-outcome.json'))
                    worker = read(worker_bound, verify)
                    require(worker.get('status') == 'passed' and worker.get('source') == ref(source)
                            and worker.get('handoff') == handoff_bound and worker.get('config') == config_bound
                            and worker.get('review') == review_bound, 'oracle worker postflight did not pass')
                    entry.update(status='passed', output=ref(output), worker_outcome=worker_bound)
                except BaseException as error:
                    entry['error'] = repr(error)
                    raise
                finally:
                    entry['completed_unix'] = time.time()
                    save(ready / (source.stem + '-oracle-outcome.json'), entry)
                oracle_receipts.append(entry)
            routing_oracle.load_reference(output, cell['case_sha256'], cell['body_sha256'])
            wrapper['independent_witness'] = ref(output)
        path = ready / (cell['id'] + '.json')
        save(path, wrapper)
        tracked_evidence(path, cell, manifest, arms, verify, prerequisites_only=True, local_oracle=local)
    return {'kind': KIND + '_witnesses', 'context': context_bound, 'handoff': handoff_bound,
            'wrappers': 42, 'oracles': oracle_receipts}


def science(config, verify, area):
    context_bound, context, manifest = cohort(config, verify)
    cases, wrappers = {}, {}
    for cell in context['cells']:
        source, local = source_case(Path(config['panel']['path']).parent, cell, verify)
        cases[cell['id']] = source
        wrapper = Path(config['output_root']) / 'witnesses/ready' / (cell['id'] + '.json')
        wrappers[cell['id']] = wrapper
        tracked_evidence(wrapper, cell, manifest, blocks.required_arms(context, cell), verify,
                         prerequisites_only=True, local_oracle=local)
    required = set().union(*(set(blocks.required_arms(context, cell)) for cell in context['cells']))
    plugins = products(config, manifest, verify)
    harness = verify.check(config['harness'])
    verify.before_producer()
    with immutable_dependencies(verify):
        return runner.run(manifest, 'C', None, harness, {arm: plugins[arm] for arm in required},
                          cases, wrappers, area / 'run', timeout=120, budget=14400,
                          prerequisites_only=True, qualification_context=context_bound)


def postflight(config, verify):
    context_bound, _, _ = cohort(config, verify)
    records = ref(Path(config['output_root']) / 'science/run/records.json')
    verify.check(records)
    with immutable_dependencies(verify):
        context, manifest, checked = blocks.qualification(context_bound, records)
        coverage = blocks.comparison_coverage(context_bound, records)
    proofs = {}
    for cell in context['cells']:
        wrapper = checked['qualified_wrappers'][cell['id']]
        path = Path(records['path']).parent / wrapper['path']
        bound = {'path': str(path.resolve(strict=True)), 'sha256': wrapper['sha256']}
        verify.check(bound)
        value = tracked_evidence(path, cell, manifest, blocks.required_arms(context, cell), verify,
                                 stage='C', selection_sha256=routing.identity(context))
        proofs[cell['id']] = {'wrapper': bound, **{key: value[key] for key in
            ('independent_witness', 'scoped_replay', 'resource_contract')}}
    verify.before_producer()
    return {'kind': KIND + '_science_postflight', 'context': context_bound, 'records': records,
            'context_sha256': routing.identity(context), 'manifest_sha256': routing.identity(manifest),
            'qualification_sha256': routing.identity(checked), 'coverage': coverage,
            'checked_wrappers': len(proofs), 'proofs': proofs, 'limitation': LIMITATION}


def stage_review(config, config_bound, review_bound, bound, verify):
    require(bound is not None, 'independent scientific postflight review required before timing freeze')
    value = read(bound, verify)
    summary = ref(Path(config['output_root']) / 'science-postflight/report.json')
    postflight_report = read(summary, verify)
    require(value.get('kind') == KIND + '_science_review' and value.get('status') == 'passed'
            and value.get('config') == config_bound and value.get('driver_review') == review_bound
            and value.get('postflight') == summary and value.get('sources') == config['sources']
            and type(value.get('completed_unix')) in (int, float)
            and postflight_report['completed_unix'] <= value['completed_unix'] <= time.time()
            and value.get('evidence'),
            'scientific postflight review missing or stale')
    for bound in value['evidence']:
        verify.check(bound)


def freeze(config, config_bound, review_bound, scientific_review, verify, area):
    stage_review(config, config_bound, review_bound, scientific_review, verify)
    context_bound, context, manifest = cohort(config, verify)
    records_bound = ref(Path(config['output_root']) / 'science/run/records.json')
    with immutable_dependencies(verify):
        _, _, records = blocks.qualification(context_bound, records_bound)
        coverage = blocks.comparison_coverage(context_bound, records_bound)
    plugins = products(config, manifest, verify)
    target = area / 'inputs'
    target.mkdir(exist_ok=False)
    cells = {}
    for cell in context['cells']:
        cid = cell['id']
        source, _ = source_case(Path(config['panel']['path']).parent, cell, verify)
        labels = {edge[key] for edge in coverage['comparisons'] if edge['cell'] == cid and edge['status'] == 'timed'
                  for key in ('candidate', 'reference')}
        arms = {}
        for arm in sorted(labels):
            destination = target / (cid + '-' + arm + '.case')
            request = runner.input_policy(manifest, arm)
            require(request is not None, 'timing admits protected Auto edges only')
            routing.prepare(source, destination, request, cell['threads'])
            runner.verify_public_request(destination, manifest, arm)
            require(ref(destination)['sha256'] == records['prepared_sha256'][cid + '/' + arm], 'prepared science/timing mismatch')
            arms[arm] = {'plugin': ref(plugins[arm]), 'input': ref(destination)}
        cells[cid] = {'source': ref(source), 'arms': arms}
    packet = {'context': context_bound, 'qualification': records_bound, 'harness': config['harness'], 'cells': cells,
              'monitor': {'validator': ref(block_host.__file__), 'policy': config['host_policy'],
                          'dependencies': [config_bound, review_bound, scientific_review, *config['sources'].values()]}}
    save(area / 'timing-config.json', packet)
    with immutable_dependencies(verify):
        spec = blocks.timing_spec(packet, paired=True)
        time_blocks.freeze(spec, area / 'manifest.json')
    verify.before_producer()
    return {'config': ref(area / 'timing-config.json'), 'manifest': ref(area / 'manifest.json')}


def collect(config, verify, area):
    manifest = Path(config['output_root']) / 'freeze-timing/manifest.json'
    verify.digest(manifest)
    original = time_blocks.load_manifest
    def load(path):
        if Path(path).resolve(strict=True) != manifest.resolve(strict=True):
            return original(path)
        # Reconstruct fresh scientific adapters. Only immutable wrapper dependency
        # digest callbacks are reused, never adapters or scientific results.
        with immutable_dependencies(verify):
            value, adapters = original(path)
        for bound in value['bindings']:
            verify.check(bound)
        verify.before_producer()
        return value, adapters
    time_blocks.load_manifest = load
    try:
        return time_blocks.collect(manifest, area / 'run')
    finally:
        time_blocks.load_manifest = original


def observer_postflight(config, verify):
    area = Path(config['output_root']) / 'collect'
    bound = ref(area / 'host/host-receipt.json')
    receipt = read(bound, verify)
    require(receipt['source'] == ref(block_host.__file__) and receipt['policy'] == config['host_policy']
            and receipt['manifest'] == ref(Path(config['output_root']) / 'freeze-timing/manifest.json')
            and receipt['records'] == ref(area / 'run/records.json')
            and receipt['events'] == ref(area / 'run/host-events.jsonl'), 'observer binding changed')
    for key in ('source', 'policy', 'manifest', 'records', 'events', 'observations'):
        verify.check(receipt[key])
    observations = block_host.read_lines(receipt['observations']['path'])
    require(observations[-1].get('kind') == 'observer_end' and observations[-1].get('runner_exit_code') == 0,
            'observer did not confirm successful child exit')
    return bound


def execute_action(action, config_bound, review_bound, start_bound, scientific_review=None):
    verify = MetadataValidation()
    result = {'kind': KIND + '_action_outcome', 'action': action, 'config': config_bound,
              'review': review_bound, 'started_unix': time.time(), 'status': 'failed'}
    area = None
    try:
        with confirmation.install(), verify:
            config = configuration(config_bound, review_bound, action, verify)
            result['sources'] = config['sources']
            area = Path(config['output_root']) / action
            require(start_bound == ref(area / 'supervisor-start.json'), 'start is not this operation supervisor start')
            start = read(start_bound, verify)
            require(start.get('kind') == KIND + '_supervisor_start' and start.get('config') == config_bound
                    and start.get('review') == review_bound and start.get('action') == action
                    and start.get('sources') == config['sources'] and start.get('scientific_review') == scientific_review,
                    'action lacks exact supervisor authorization')
            if action != 'exposed':
                lock = ROOT / 'target/tmp/auto-routing/many/confirmation-repair-execution.json'
                require(read(ref(lock), verify) == {'config': config_bound, 'review': review_bound,
                    'declaration': config['declaration']}, 'action outside the sole reviewed confirmation version')
            save(area / 'start.json', {**result, 'supervisor_start': start_bound})
            prior(config, config_bound, review_bound, action, verify)
            if action == 'exposed':
                report = exposed(config, verify, review_bound)
            elif action == 'derive':
                context = confirmation.derive(config['declaration'], config['panel'], config['generation'])
                declaration = read(config['declaration'], verify, sealed=True)
                require(declaration['config']['sources'] == config['sources'], 'execution producer sources differ from declaration')
                save(area / 'context.json', routing.seal(context))
                verify.before_producer()
                report = {'context': ref(area / 'context.json')}
            elif action == 'witnesses':
                report = witnesses(config, config_bound, review_bound, verify, area)
            elif action == 'science':
                report = science(config, verify, area)
            elif action == 'science-postflight':
                report = postflight(config, verify)
            elif action == 'freeze-timing':
                report = freeze(config, config_bound, review_bound, scientific_review, verify, area)
            elif action == 'collect':
                report = collect(config, verify, area)
            else:
                receipt = observer_postflight(config, verify)
                with immutable_dependencies(verify):
                    report = deployment.decide(routing.read_json(Path(config['output_root']) / 'freeze-timing/timing-config.json'),
                        ref(Path(config['output_root']) / 'freeze-timing/manifest.json'),
                        ref(Path(config['output_root']) / 'collect/run/records.json'), receipt)
                save(area / 'decision.json', report)
                report = {'decision': ref(area / 'decision.json')}
                verify.before_producer()
        report.update(metadata=verify.report(), completed_unix=time.time())
        save(area / 'report.json', report)
        outputs = {'report': ref(area / 'report.json')}
        for name in ('context.json', 'handoff.json', 'timing-config.json', 'manifest.json', 'run/records.json', 'decision.json'):
            if (area / name).exists():
                outputs[name] = ref(area / name)
        result.update(status='passed', outputs=outputs)
    except BaseException as error:
        result['error'] = repr(error)
        raise
    finally:
        result.update(completed_unix=time.time(), metadata=verify.report())
        if area is not None:
            save(area / 'outcome.json', result)


def supervise(action, config_bound, review_bound, scientific_review=None):
    verify = ByteVerificationPass()
    area, result = None, None
    try:
        with verify:
            config = configuration(config_bound, review_bound, action, verify)
            prior(config, config_bound, review_bound, action, verify)
            helper = owned(config)
            processes = helper.processes()
            require(os.getpid() in processes, 'supervisor process birth identity unavailable')
            if action != 'exposed':
                lock = ROOT / 'target/tmp/auto-routing/many/confirmation-repair-execution.json'
                expected = {'config': config_bound, 'review': review_bound, 'declaration': config['declaration']}
                if action == 'derive':
                    save(lock, expected)
                else:
                    require(read(ref(lock), verify) == expected, 'confirmation version already belongs to another execution')
            area = Path(config['output_root']) / action
            area.mkdir(parents=True, exist_ok=True)
            start = {'kind': KIND + '_supervisor_start', 'action': action, 'config': config_bound,
                     'review': review_bound, 'scientific_review': scientific_review, 'sources': config['sources'],
                     'started_unix': time.time(), 'budget_s': BOUNDS[action], 'pid': os.getpid(),
                     'birth': processes[os.getpid()]['birth'], 'process': processes[os.getpid()],
                     'argv': [sys.executable, str(Path(sys.argv[0]).absolute()), *sys.argv[1:]]}
            save(area / 'supervisor-start.json', start)
            result = {**start, 'kind': KIND + '_supervisor_outcome', 'status': 'failed'}
            command = [sys.executable, str(Path(__file__).resolve()), 'action', '--action', action,
                       '--config', json.dumps(config_bound), '--review', json.dumps(review_bound),
                       '--start', json.dumps(ref(area / 'supervisor-start.json'))]
            if scientific_review:
                command += ['--scientific-review', json.dumps(scientific_review)]
            if action == 'collect':
                command = [sys.executable, str(Path(block_host.__file__).resolve()),
                           '--manifest', str(Path(config['output_root']) / 'freeze-timing/manifest.json'),
                           '--policy', config['host_policy']['path'], '--events', str(area / 'run/host-events.jsonl'),
                           '--records', str(area / 'run/records.json'), '--output', str(area / 'host'), '--', *command]
            result['command'] = command
            previous = {sig: signal.signal(sig, helper.interrupted) for sig in (signal.SIGINT, signal.SIGTERM)}
            try:
                verify.before_producer()
                result['process_result'] = helper.run_owned(command, area / 'supervisor.log', BOUNDS[action], result)
            finally:
                for sig, handler in previous.items():
                    signal.signal(sig, handler)
            child_bound = ref(area / 'outcome.json')
            child = read(child_bound, verify)
            require(child.get('status') == 'passed' and child.get('config') == config_bound
                    and child.get('review') == review_bound and child.get('sources') == config['sources']
                    and child.get('action') == action, 'child failed or returned different bindings')
            for bound in child['outputs'].values():
                verify.check(bound)
            result.update(child_outcome=child_bound, outputs={})
            if action == 'collect':
                result['outputs']['observer'] = observer_postflight(config, verify)
        # The context's final full-byte check has completed BEFORE a success can
        # be persisted. A failed automatic finish is a failed supervisor outcome.
        result['status'] = 'passed'
    except BaseException as error:
        if result is not None:
            result['error'] = repr(error)
        raise
    finally:
        if result is not None:
            result.update(completed_unix=time.time(), metadata=verify.report())
            save(area / 'supervisor-outcome.json', result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('supervise', 'action', 'oracle-worker'))
    parser.add_argument('--config', type=json.loads, required=True, help='canonical {path,sha256} JSON binding')
    parser.add_argument('--review', type=json.loads, required=True)
    parser.add_argument('--action', choices=tuple(BOUNDS))
    parser.add_argument('--start', type=json.loads)
    parser.add_argument('--scientific-review', type=json.loads)
    parser.add_argument('--handoff', type=json.loads)
    parser.add_argument('--source', type=json.loads)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    if args.mode == 'supervise':
        require(args.execute and args.action is not None, 'explicit --execute and action required')
        supervise(args.action, args.config, args.review, args.scientific_review)
    elif args.mode == 'action':
        require(args.action is not None and args.start is not None, 'supervisor start required')
        execute_action(args.action, args.config, args.review, args.start, args.scientific_review)
    else:
        require(args.handoff and args.source and args.output, 'sealed parent witness handoff required')
        witness_worker(args.handoff, args.source, args.output, args.config, args.review)


if __name__ == '__main__':
    main()
