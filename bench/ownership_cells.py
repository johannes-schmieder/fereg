#!/usr/bin/env python3
"""Frozen ownership/cell development panels, without qualification or activation.

prepare generates twelve fresh historical shapes and independent references;
plan binds existing artifacts and a control proof; run performs one finite
panel; report preserves all observations; diagnose uses a separate <=600s
budget and never contributes scored observations. There is no retry/resume.
"""
from __future__ import annotations

import argparse
import copy
import json
import math
import os
from pathlib import Path
import platform
import random
import re
import shutil
import socket
import statistics
import time

import numpy as np
import cmgschur_cases as cases
import qualify as q
import routing
import routing_cases
import routing_oracle

FAMILIES = ('weak', 'high-degree-weak', 'dense', 'repeated', 'nonmovers', 'hubs',
            'disconnected', 'weight-dynamic', 'wide', 'inactive-zero', 'low', 'easy')
DATA_SEED = 2026106301
REFERENCE_REVISION = 'c16c0b5555338d8b20b184e6e51ad2acbe6a4e6e'
ORDER_SEEDS = {1: 2026106303, 2: 2026106304, 3: 2026106305}
OWNERSHIP = ('none', 'moments', 'scores', 'both')
KIND = 'ownership-cells-development-v1'
COVERAGE_KIND = 'ownership-moments-coverage-v1'
require = q.require


def save_new(path, value):
    path = Path(path)
    require(not path.exists(), 'refuse overwrite: ' + str(path))
    q.write(path, value)


def columns(family):
    """The twelve historical shape definitions, with a new recorded RNG stream."""
    require(family in FAMILIES, 'unknown family')
    seed = DATA_SEED + FAMILIES.index(family)
    weights, intercept = None, True
    if family == 'high-degree-weak':
        values = cases.high_degree_case('d30-weak', 1100, 300, 32, seed)
    elif family in {'wide', 'low', 'inactive-zero'}:
        values = cases.additional_case('orthogonal' if family == 'inactive-zero' else 'wide', 1000, 100, 20, seed)
        intercept = family != 'inactive-zero'
    elif family in {'disconnected', 'weight-dynamic'}:
        values, weights, _, intercept = routing_cases.extended_columns(family, seed, 1000, 100, 20)
    else:
        values = cases.synthetic('weak' if family == 'weak' else 'short' if family == 'easy' else 'long',
                                 1000, 100, 4 if family == 'easy' else 20, seed)
        if family == 'dense':
            values = cases.high_degree_case('d30-mixed', 1000, 100, 32, seed)
        elif family == 'hubs':
            values = cases.high_degree_case('d60-stress', 1000, 100, 64, seed)
        elif family == 'nonmovers':
            values[-1][2000:] = np.repeat(np.arange(100, 1000) % 100, 20)
        elif family == 'repeated':
            values = [np.repeat(value, 4) for value in values]
    require(0 < len(values[0]) <= 100000 and all(len(v) == len(values[0]) for v in values), 'shape exceeds fixed bound')
    return values, weights, intercept


def sources():
    return {**q.execution_sources(), 'ownership_cells': Path(__file__).resolve(),
            **{name: Path(__file__).with_name(name + '.py')
               for name in ('ownership_moments', 'ownership_moments_cases', 'ownership_moments_repair',
                            'ownership_panel3')}}


def settings(plan):
    if plan['kind'] == COVERAGE_KIND:
        return plan['settings']
    return [dict(cell=f'{family}-t{worker}', family=family, workers=worker)
            for family in FAMILIES for worker in (1, 4)]


def prepare(generator, output):
    output = routing_cases.scratch(Path(output))
    output.mkdir(parents=True, exist_ok=False)
    generator = Path(generator).resolve(strict=True)
    closure = {name: q.digest(path) for name, path in sources().items()}
    items, files = [], {}
    for family in FAMILIES:
        target = output / (family + '.case')
        values, weights, intercept = columns(family)
        cases.write_case(generator, target, values, include_intercept=intercept, weights=weights)
        if family == 'low':
            prior = output / 'low-fast.case'
            target.rename(prior)
            routing_cases.copy_import(prior, target, low=True)
            prior.unlink()
        meta = routing_cases.inspect_case(target, family)
        meta.update(family=family, seed=DATA_SEED, derived_seed=DATA_SEED + FAMILIES.index(family),
                    generator_sha256=q.digest(generator), generator_sources=closure,
                    case_sha256=q.digest(target), **cases.graph_summary(values))
        metadata = target.with_suffix('.json')
        save_new(metadata, meta)
        oracle = output / (family + '.oracle.json')
        routing_oracle.write_reference(target, oracle)
        _, witnesses = routing_oracle.load_reference(oracle, meta['case_sha256'], meta['body_sha256'])
        for path in [target, metadata, *witnesses]:
            files[path.name] = q.digest(path)
        items.append(dict(id=family, family=family, input=target.name, metadata=metadata.name,
                          oracle={'kind': 'routing', 'path': oracle.name}, memory_limit_bytes=None))
    require(closure == {name: q.digest(path) for name, path in sources().items()}, 'generator source changed')
    panel = dict(schema=1, kind=KIND + '-inputs', seed=DATA_SEED, families=list(FAMILIES),
                 generator_sha256=q.digest(generator), generator_sources=closure, cases=items, files=files)
    save_new(output / 'panel.json', panel)
    return panel


def mechanisms(panel, ownership):
    require(type(panel) is int and panel in ORDER_SEEDS and ownership in OWNERSHIP, 'unknown panel/ownership')
    if panel == 1:
        require(ownership == 'none', 'panel1 tests ownership independently')
        return {'B': ('none', 'none'), 'moments': ('moments', 'none'),
                'scores': ('scores', 'none'), 'both': ('both', 'none')}
    if panel == 2:
        return {'B': ('none', 'none'), 'O': (ownership, 'none'), 'S': ('none', 'eager'), 'O+S': (ownership, 'eager')}
    return {'B': ('none', 'none'), 'O': (ownership, 'none'),
            'O+eager': (ownership, 'eager'), 'O+deferred': (ownership, 'deferred')}


def token(mechanism):
    ownership, cell = mechanism
    if cell == 'none':
        return None if ownership == 'none' else 'ownership_' + ownership
    return 'cells_' + cell + ('' if ownership == 'none' else '_' + ownership)


def aliases(arms):
    seen, result = {}, {}
    for role, mechanism in arms.items():
        result[role] = seen.setdefault(tuple(mechanism), role)
    return result


def physical_arms(plan):
    # JSON objects are serialized with sorted keys; protocol order is explicit.
    return list(dict.fromkeys(plan['aliases'][role] for role in mechanisms(plan['panel'], plan['ownership'])))


def schedule(plan, diagnostic=False):
    if plan['kind'] == COVERAGE_KIND:
        from ownership_moments import schedule as coverage_schedule
        return coverage_schedule(plan, diagnostic)
    roles = physical_arms(plan)
    cells = [f'{f}-t{w}' for f in FAMILIES for w in (1, 4)]
    if diagnostic:
        return [dict(cell=cell, arm=arm, phase='diagnostic', block=i, mirror=0)
                for cell in cells for arm in roles for i in range(plan['diagnostic_repeats'])]
    rng = random.Random(plan['order_seed'])
    result = [dict(cell=cell, arm=arm, phase='warmup', block=-1, mirror=0)
              for cell in cells for arm in roles]
    for block in range(6):
        for cell in rng.sample(cells, len(cells)):
            forward = rng.sample(roles, len(roles))
            for mirror, order in enumerate((forward, list(reversed(forward)))):
                result.extend(dict(cell=cell, arm=arm, phase='scored', block=block, mirror=mirror) for arm in order)
    return result


def prepare_request(source, destination, mechanism, workers):
    """One sanctioned token; all data/model bytes remain identical."""
    selected = token(mechanism)
    with Path(source).open('rb') as stream, Path(destination).open('xb') as out:
        shape, arguments = q.case_header(stream)
        require(arguments[0] == b'estimate-ols', 'only OLS requests are supported')
        fields = [a.split(b'=', 1)[0] for a in arguments[1:]]
        require(len(fields) == len(set(fields)) and not any(a.startswith((b'inject=', b'break_marker_token=')) for a in arguments),
                'source injection/fault/duplicate option')
        updates = {'solver': 'auto', 'preconditioner': 'auto', 'threads': str(workers)}
        arguments = [a for a in arguments if a.split(b'=', 1)[0].decode() not in updates]
        arguments += [f'{k}={v}'.encode() for k, v in updates.items()]
        if selected:
            arguments.append(('inject=' + selected).encode())
        shape[2] = len(arguments)
        out.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, shape)) + '\n').encode())
        for value in arguments:
            out.write(str(len(value)).encode() + b'\n' + value + b'\n')
        shutil.copyfileobj(stream, out, 1 << 20)
    require(routing_cases.inspect_case(source)['body_sha256'] == routing_cases.inspect_case(destination)['body_sha256'],
            'preparation changed source model/body')


def validate(plan):
    if plan['kind'] == COVERAGE_KIND:
        from ownership_moments import validate as validate_coverage
        return validate_coverage(plan)
    require(plan['schema'] == 1 and plan['kind'] == KIND and plan['platform'] in ('Linux', 'Darwin'), 'invalid plan')
    expected = {k: list(v) for k, v in mechanisms(plan['panel'], plan['ownership']).items()}
    require(plan['arms'] == expected and plan['aliases'] == aliases(expected), 'arm meaning/alias changed')
    require(plan['order_seed'] == ORDER_SEEDS[plan['panel']] and plan['data_seed'] == DATA_SEED
            and plan['threads'] == [1, 4] and plan['blocks'] == 6 and plan['warmups'] == 1
            and plan['timeout_s'] == 120 and plan['budget_s'] == 3600 and plan['diagnostic_budget_s'] == 600,
            'fixed protocol changed')
    require(type(plan['diagnostic_repeats']) is int and 1 <= plan['diagnostic_repeats'] <= 3, 'diagnostic cap changed')
    require([c['id'] for c in plan['cases']] == list(FAMILIES)
            and all(c['family'] == c['id'] and c['memory_limit_bytes'] is None
                    and c['oracle']['kind'] == 'routing' for c in plan['cases']), 'family/oracle coverage changed')
    require(plan['schedule'] == schedule(plan) and plan['call_cap'] == 24 * len(set(plan['aliases'].values())) * 13,
            'schedule/call cap changed')
    require(plan['diagnostic_schedule'] == schedule(plan, True), 'diagnostic schedule changed')
    require(plan['platform'] != 'Linux' or plan['observer'] == 'whole-host', 'Linux needs actual whole-host isolation')
    require(plan['platform'] != 'Darwin' or plan['observer'] == 'desktop', 'Mac observer changed')
    require(plan['panel'] != 3 or plan['conditional_authorization'] is True
            and plan['ownership'] == 'both' and isinstance(plan['panel2_result'], dict)
            and set(plan['panel2_result']) == {'results', 'assessment'},
            'panel3 is not automatically authorized')


def bind_inputs_artifacts(cfg, config_path, output):
    """Shared immutable inputs/artifacts for correctness and development panels."""
    files = {}
    def bind(path):
        path = (config_path.parent / path).resolve(strict=True)
        name = os.path.relpath(path, output.parent)
        files[name] = q.digest(path)
        return name
    panel_path = (config_path.parent / cfg['inputs']).resolve(strict=True)
    panel = q.read(panel_path)
    require(panel['kind'] == KIND + '-inputs' and panel['seed'] == DATA_SEED and panel['families'] == list(FAMILIES),
            'wrong generated panel')
    input_binding = bind(cfg['inputs'])
    items = []
    bodies = set()
    for item in panel['cases']:
        copied = copy.deepcopy(item)
        for name in (item['input'], item['metadata'], item['oracle']['path']):
            require(q.digest(q.resolve(panel_path, name)) == panel['files'][name], 'generated input changed')
        copied['input'] = bind(str(q.resolve(panel_path, item['input'])))
        copied['metadata'] = bind(str(q.resolve(panel_path, item['metadata'])))
        copied['oracle']['path'] = bind(str(q.resolve(panel_path, item['oracle']['path'])))
        meta = q.read(q.resolve(panel_path, item['metadata']))
        observed = routing_cases.inspect_case(q.resolve(panel_path, item['input']), item['id'])
        require(meta['seed'] == DATA_SEED and meta['derived_seed'] == DATA_SEED + FAMILIES.index(item['id'])
                and meta['generator_sha256'] == panel['generator_sha256']
                and meta['generator_sources'] == panel['generator_sources']
                and meta['case_sha256'] == panel['files'][item['input']] and meta['body_sha256'] == observed['body_sha256']
                and observed['absorbed_terms'] == 2 and observed['categorical_intercepts'], 'input provenance/model changed')
        bodies.add(meta['body_sha256'])
        _, witness_files = routing_oracle.load_reference(q.resolve(panel_path, item['oracle']['path']),
                                                        meta['case_sha256'], meta['body_sha256'])
        for path, digest in witness_files.items():
            require(q.digest(path) == digest, 'oracle changed')
            bind(str(path))
        items.append(copied)
    require(len(bodies) == 12, 'generated bodies collide')
    artifacts = {}
    for role in ('candidate', 'reference'):
        art = cfg[role]
        require(set(art) == {'path', 'revision', 'source_manifest', 'build'}
                and re.fullmatch('[0-9a-f]{40}', art['revision']) and isinstance(art['build'], str) and art['build'],
                'artifact needs actual bytes/revision/source manifest/build settings')
        artifacts[role] = {**art, 'path': bind(art['path']), 'source_manifest': bind(art['source_manifest'])}
    require(artifacts['reference']['revision'] == REFERENCE_REVISION, 'accepted current reference changed')
    for role, art in artifacts.items():
        manifest = q.read(q.resolve(output, art['source_manifest']))
        require(manifest['fereg'] == art['revision'], 'artifact source revision mismatch')
    return files, bind, input_binding, items, artifacts


def freeze(config_path, output):
    config_path, output = Path(config_path).resolve(), Path(output).resolve()
    cfg = q.read(config_path)
    required = {'platform', 'panel', 'ownership', 'inputs', 'harness', 'candidate', 'reference', 'control_proof',
                'diagnostic_repeats', 'conditional_authorization', 'panel2_result'}
    require(set(cfg) - {'control_execution_source'} == required, 'configuration fields differ: ' + str(sorted(required)))
    files, bind, input_binding, items, artifacts = bind_inputs_artifacts(cfg, config_path, output)
    proof_path = bind(cfg['control_proof'])
    proof = q.read(q.resolve(output, proof_path))
    require(proof['status'] == 'passed' and proof['unmodified_controls_exact'] is True
            and proof['candidate_sha256'] == files[artifacts['candidate']['path']]
            and proof['reference_sha256'] == files[artifacts['reference']['path']]
            and proof['inputs_sha256'] == files[input_binding]
            and proof['kind'] == KIND + '-controls' and proof['platform'] == cfg['platform']
            and proof['native_calls_started'] == 48 and proof['failures'] == [] and proof['retries'] == 0
            and proof['elapsed_s'] <= 600 and proof['schedule'] == controls_schedule()
            and [{key: call[key] for key in ('family', 'cell', 'workers', 'arm')} for call in proof['calls']] == controls_schedule()
            and all(call['status'] == 'passed' and call['oracle'] == 'passed' for call in proof['calls']),
            'missing exact uninjected control proof')
    control_source = None
    if cfg.get('control_execution_source') is not None:
        require(cfg['panel'] == 3, 'control source amendment is only for panel3')
        control_source = bind(cfg['control_execution_source'])
    from ownership_panel3 import control_sources
    control_sources(proof, {name: q.digest(path) for name, path in sources().items()},
                    q.resolve(output, control_source) if control_source else None)
    panel2 = None
    if cfg['panel'] == 3:
        require(isinstance(cfg['panel2_result'], dict)
                and set(cfg['panel2_result']) == {'results', 'assessment'}, 'panel3 evidence bindings missing')
        panel2 = {name: bind(path) for name, path in cfg['panel2_result'].items()}
    else:
        require(cfg['panel2_result'] is None, 'prior panel evidence is only used by panel3')
    arms = {k: list(v) for k, v in mechanisms(cfg['panel'], cfg['ownership']).items()}
    result = dict(schema=1, kind=KIND, platform=cfg['platform'], panel=cfg['panel'], ownership=cfg['ownership'],
                  data_seed=DATA_SEED, order_seed=ORDER_SEEDS[cfg['panel']], threads=[1, 4], blocks=6, warmups=1,
                  timeout_s=120, budget_s=3600, diagnostic_budget_s=600, diagnostic_repeats=cfg['diagnostic_repeats'],
                  observer='whole-host' if cfg['platform'] == 'Linux' else 'desktop', arms=arms, aliases=aliases(arms),
                  cases=items, inputs=input_binding, harness=bind(cfg['harness']), artifacts=artifacts,
                  control_proof=proof_path, panel2_result=panel2, conditional_authorization=cfg['conditional_authorization'],
                  execution_sources={n: q.digest(p) for n, p in sources().items()})
    if control_source:
        result['control_execution_source'] = control_source
    result['schedule'] = schedule(result)
    result['diagnostic_schedule'] = schedule(result, True)
    result['call_cap'] = len(result['schedule'])
    result['files'] = files
    validate(result)
    if cfg['panel'] == 3:
        from ownership_panel3 import prior_result
        prior_result(result, output)
    save_new(output, result)
    return result


def verify_files(plan, manifest):
    validate(plan)
    return verify_bound_files(plan, manifest)


def verify_bound_files(plan, manifest):
    require(plan['execution_sources'] == {n: q.digest(p) for n, p in sources().items()}, 'execution source changed')
    paths = [*sources().values(), Path(manifest)]
    for name, expected in plan['files'].items():
        path = q.resolve(manifest, name)
        require(q.digest(path) == expected, 'frozen file changed: ' + name)
        paths.append(path)
    return {p: q.stamp(p) for p in paths}


def controls_schedule():
    return [dict(family=family, cell=f'{family}-t{worker}', workers=worker, arm=arm)
            for family in FAMILIES for worker in (1, 4) for arm in ('reference', 'candidate')]


def controls(config_path, output, *, execute=None):
    """Exactly 48 uninjected correctness calls, without performance selection."""
    started = time.monotonic()
    config_path = Path(config_path).resolve()
    output = routing_cases.scratch(Path(output))
    require(not output.exists(), 'control output exists; no retries/resume')
    output.mkdir(parents=True)
    result_path, proof_path = output / 'results.json', output / 'control-proof.json'
    sequence = controls_schedule()
    result = dict(schema=1, kind=KIND + '-controls', status='running', calls=[], failures=[],
                  schedule=sequence, native_calls_started=0, call_cap=48, budget_s=600, timeout_s=120,
                  retries=0, elapsed_s=0, unmodified_controls_exact=False, performance_claim=False,
                  hostname=socket.gethostname())
    def persist():
        result['elapsed_s'] = time.monotonic() - started
        q.write(result_path, result)
    persist()
    execute = q.run_process if execute is None else execute
    try:
        with q.Deadline(600, started) as deadline:
            cfg = q.read(config_path)
            require(set(cfg) == {'platform', 'inputs', 'harness', 'candidate', 'reference'}, 'invalid controls configuration')
            require(cfg['platform'] == platform.system() and cfg['platform'] in ('Linux', 'Darwin'), 'wrong control platform')
            manifest = output / 'control-plan.json'
            files, bind, inputs, items, artifacts = bind_inputs_artifacts(cfg, config_path, manifest)
            plan = dict(schema=1, kind=KIND + '-control-plan', platform=cfg['platform'], cases=items,
                        artifacts=artifacts, inputs=inputs, harness=bind(cfg['harness']), config=bind(str(config_path)),
                        files=files, schedule=sequence, budget_s=600, timeout_s=120,
                        execution_sources={name: q.digest(path) for name, path in sources().items()})
            save_new(manifest, plan)
            stamps = verify_bound_files(plan, manifest)
            result.update(platform=cfg['platform'], plan=plan, manifest_sha256=routing.identity(plan),
                          candidate_sha256=files[artifacts['candidate']['path']],
                          reference_sha256=files[artifacts['reference']['path']], inputs_sha256=files[inputs])
            requests, metadata, snapshots = {}, {}, {}
            for item in items:
                metadata[item['id']] = q.metadata(plan, item, manifest)
                for worker in (1, 4):
                    cell = f"{item['id']}-t{worker}"
                    directory = output / cell
                    directory.mkdir()
                    request = directory / 'uninjected.case'
                    prepare_request(q.resolve(manifest, item['input']), request, ('none', 'none'), worker)
                    requests[cell] = request
                    stamps[request] = q.stamp(request)
            persist()
            for entry in sequence:
                deadline.check()
                require(all(q.stamp(p) == v for p, v in stamps.items()), 'bound control source/input changed')
                cell, arm = entry['cell'], entry['arm']
                meta = metadata[entry['family']]
                target = output / cell / (arm + '.json')
                log = target.with_suffix('.log')
                call = {**entry, 'status': 'started', 'target': str(target.relative_to(output)),
                        'log': str(log.relative_to(output)), 'input_sha256': q.digest(requests[cell])}
                result['calls'].append(call)
                result['native_calls_started'] += 1
                persist()
                env = ['env', *[v for key in q.ENVIRONMENT for v in ('-u', key)],
                       *[key + '=1' for key in q.THREAD_ENV], 'FEREG_SOLVER_TRACE=1']
                resources = execute([*env, str(q.resolve(manifest, plan['harness'])),
                                     str(q.resolve(manifest, artifacts[arm]['path'])), '--replay',
                                     str(requests[cell]), str(target)], log, min(120, deadline.check()))
                raw, columns, detail = q.read_native(target, log, meta)
                require(detail['seconds'] <= 120 and detail['solver_rows'][0][0] == q.cmgschur.EXPECTED_SOLVER['auto'],
                        'control timeout/requested Auto identity changed')
                detail.update(worker_signature=q.worker_signature(raw), model_signature=q.science.model_discrete_sha256(raw),
                              **q.science.resource_diagnostics(log.read_text(), None, False, {}))
                base = output / cell / ('reference-' + arm)
                retained = dict(path=str(base.relative_to(output)), **q.retained_snapshot(base, raw, columns, meta))
                call.update(detail, raw_result=raw, columns_sha256=q.digest(columns),
                            result_sha256=q.digest(target), log_sha256=q.digest(log),
                            maxrss_bytes=resources['maxrss_bytes'], reference=retained)
                if arm == 'candidate':
                    before, _ = snapshots[cell, 'reference']
                    require(before['fingerprint'] == detail['fingerprint']
                            and before['worker_signature'] == detail['worker_signature'], 'uninjected exact control mismatch')
                if entry['workers'] == 4:
                    before, prior_base = snapshots[entry['family'] + '-t1', arm]
                    q.compare_saved(prior_base, base, meta)
                    require(before['worker_signature'] == detail['worker_signature'], 'control cross-worker decisions changed')
                snapshots[cell, arm] = detail, base
                call.update(status='passed', oracle='passed')
                deadline.check()
                persist()
                columns.unlink()
            require(len(result['calls']) == result['native_calls_started'] == 48, 'incomplete controls')
            require(all(q.stamp(p) == v for p, v in stamps.items()), 'final control source/input drift')
            deadline.check()
            result.update(status='passed', unmodified_controls_exact=True)
            persist()
            deadline.check()
            save_new(proof_path, result)
            deadline.check()
    except BaseException as error:
        if proof_path.exists():
            proof_path.unlink()  # A deadline interrupted publication; no usable proof.
        result.update(status='failed', unmodified_controls_exact=False)
        result['failures'].append(dict(type=type(error).__name__, message=str(error)))
        if result['calls']:
            call = result['calls'][-1]
            for field, name in [('result_sha256', 'target'), ('log_sha256', 'log')]:
                path = output / call[name]
                if path.exists():
                    call[field] = q.digest(path)
        persist()
        raise
    return result


def hierarchies(detail):
    require(len(detail['route_trace']) == len(detail['route_hierarchy']), 'route/hierarchy trace length differs')
    return [hierarchy for route, hierarchy in zip(detail['route_trace'], detail['route_hierarchy']) if route['installed'] == '1']


def compare_warmups(plan, snapshots, meta):
    """Exact ownership; cells have independent mathematics and exact model decisions."""
    logical = {arm: snapshots[alias] for arm, alias in plan['aliases'].items()}
    if plan['panel'] == 3:
        from ownership_panel3 import compare_actions
        compare_actions(logical['O+deferred'][2], logical['O'][2])
    for arm, current in logical.items():
        baseline = logical['B']
        q.cmgschur.scientific_comparison(baseline[0], current[0], baseline[1], current[1], meta)
        require(q.science.model_discrete_sha256(baseline[0]) == q.science.model_discrete_sha256(current[0]), 'cell model decisions changed')
        for other, before in logical.items():
            if plan['arms'][arm][1] == plan['arms'][other][1]:
                require(current[2]['fingerprint'] == before[2]['fingerprint'], 'ownership changed exact science/solver signature')
            a, b = hierarchies(current[2]), hierarchies(before[2])
            if a and b:
                require(a == b, 'original graph hierarchy changed when both arms build')


def ownership_diagnostic(log, mechanism):
    """Attest actual work only in the separately budgeted phase diagnostic."""
    events = []
    for line in log.splitlines():
        if not line.startswith('FEREG_OWNERSHIP|'):
            continue
        parts = [part.split('=', 1) for part in line.split('|')[1:]]
        require(all(len(part) == 2 for part in parts) and len({p[0] for p in parts}) == len(parts),
                'malformed ownership diagnostic')
        require(all(re.fullmatch(r'0|[1-9][0-9]*', p[1]) for p in parts), 'invalid ownership counters')
        events.append({key: int(value) for key, value in parts})
    moments = [event for event in events if set(event) == {'unused_moment_bytes'}]
    constructed = [event for event in events if set(event) == {
        'constructed_moment_rows', 'constructed_moment_columns', 'moment_workspace_bound_bytes'}]
    scores = [event for event in events if set(event) == {'score_admitted', 'avoided_packed_elements',
              'avoided_packed_bytes', 'retained_matrix_capacity_bytes', 'generated_columns', 'kept_columns'}]
    require(len(scores) == 1 and len(moments) <= 1 and len(constructed) <= 1
            and len(events) == len(scores) + len(moments) + len(constructed),
            'missing/unknown ownership diagnostic')
    require(all(all(value > 0 for value in event.values()) for event in constructed)
            and not (constructed and any(event['unused_moment_bytes'] for event in moments)),
            'constructed and omitted moment work is inconsistent')
    score = scores[0]
    require(score['score_admitted'] in (0, 1) and score['kept_columns'] <= score['generated_columns']
            and score['avoided_packed_bytes'] == 8 * score['avoided_packed_elements'], 'inconsistent ownership counters')
    if score['score_admitted']:
        require(mechanism[0] in ('scores', 'both') and score['retained_matrix_capacity_bytes'] > 0,
                'unrequested score ownership admitted')
    else:
        require(score['avoided_packed_bytes'] == score['retained_matrix_capacity_bytes'] == 0,
                'nonadmitted ownership claims avoided work')
    require(not moments or mechanism[0] in ('moments', 'both'), 'unrequested moments omission')
    return dict(events=events, requested_ownership=mechanism[0],
                limitation='Requested ownership may be ineligible; counters attest actual admission, not speed.')


def cell_diagnostic(log, mechanism):
    """Retain construction state, including skipped, refused or reused setup."""
    events = []
    fields = {'event', 'attempted', 'admitted', 'refusal', 'cells', 'retained_bytes', 'peak_bytes'}
    for line in log.splitlines():
        if not line.startswith('FEREG_CELLS|'):
            continue
        parts = [part.split('=', 1) for part in line.split('|')[1:]]
        require(all(len(part) == 2 for part in parts) and len(parts) == len(fields)
                and {p[0] for p in parts} == fields, 'malformed cell diagnostic')
        event = dict(parts)
        require(event['event'] in ('bound', 'attempt', 'reuse')
                and event['refusal'] in ('None', 'Some(MemoryBudget)', 'Some(Numerical)'),
                'unknown cell event/refusal')
        for key in fields - {'event', 'refusal'}:
            require(re.fullmatch(r'0|[1-9][0-9]*', event[key]), 'invalid cell counter')
            event[key] = int(event[key])
        require(event['attempted'] in (0, 1) and event['admitted'] in (0, 1)
                and event['peak_bytes'] >= event['retained_bytes'], 'inconsistent cell counters')
        if event['admitted']:
            require(event['attempted'] == 1 and event['refusal'] == 'None'
                    and event['cells'] > 0 and event['retained_bytes'] > 0, 'invalid cell admission')
        else:
            require(event['cells'] == event['retained_bytes'] == 0, 'nonadmitted cells claim storage')
        events.append(event)
    require(not events or mechanism[1] != 'none', 'unrequested cell construction')
    if events:
        bound = events[0]
        require(bound == dict(event='bound', attempted=0, admitted=0, refusal='None',
                              cells=0, retained_bytes=0, peak_bytes=0), 'missing clean cell binding')
        if len(events) > 1:
            attempt = events[1]
            require(attempt['event'] == 'attempt' and attempt['attempted'] == 1,
                    'missing single cell attempt')
            require(attempt['admitted'] == 1 or attempt['refusal'] != 'None',
                    'nonadmitted cell attempt lacks refusal')
            require(all(event == {**attempt, 'event': 'reuse'} for event in events[2:]),
                    'cell state changed or constructed twice')
    return dict(requested_cells=mechanism[1], events=events, bound=bool(events),
                constructed=any(event['admitted'] for event in events),
                limitation='Binding, admission and reuse state do not prove application or a complete-command speedup.')


def run(manifest, output, *, diagnostic=False, scored_result=None, preflight_result=None,
        execute=None, monitor_factory=None):
    """One attempt only. Persist the started call before entering native code."""
    started = time.monotonic()
    manifest, output = Path(manifest).resolve(), routing_cases.scratch(Path(output))
    require(not output.exists(), 'run output exists; no retries/resume')
    plan = q.read(manifest)
    validate(plan)
    coverage = plan['kind'] == COVERAGE_KIND
    reused = 45 if coverage and diagnostic and 'preflight_reuse' in plan else 0
    if reused:
        started -= plan['preflight_reuse']['charged_seconds']
    require(plan['platform'] == platform.system(), 'wrong execution platform')
    output.mkdir(parents=True)
    sequence = plan['diagnostic_schedule'] if diagnostic else plan['schedule']
    result = dict(schema=1, kind=plan['kind'], platform=plan['platform'], panel=plan['panel'], hostname=socket.gethostname(),
                  manifest_sha256=routing.identity(plan), status='running', native_calls_started=0, calls=[], failures=[],
                  cells={}, elapsed_s=0, diagnostic=diagnostic, activation_qualified=False, ownership=plan['ownership'],
                  candidate_sha256=plan['files'][plan['artifacts']['candidate']['path']],
                  inputs_sha256=plan['files'][plan['inputs']],
                  call_cap=len(sequence), budget_s=plan['diagnostic_budget_s'] if diagnostic else plan['budget_s'],
                  exclusions=[], retries=0)
    host = None
    path = output / 'results.json'
    def persist():
        result['elapsed_s'] = time.monotonic() - started
        if reused:
            result['native_calls_executed_here'] = max(0, result['native_calls_started'] - reused)
        q.write(path, result)
    persist()
    execute = q.run_process if execute is None else execute
    try:
        with q.Deadline(result['budget_s'], started) as deadline:
            stamps = verify_files(plan, manifest)
            historical = None
            if plan['panel'] == 3:
                from ownership_panel3 import prior_result
                historical = prior_result(plan, manifest)
            scored = prerequisite = None
            if coverage and not diagnostic:
                from ownership_moments import validate_preflight
                require(preflight_result is not None, 'coverage timing requires actual preflight evidence')
                require(Path(preflight_result).resolve() == output.parent / plan['preflight_output'] / 'results.json',
                        'coverage preflight must be the bound sibling output')
                prerequisite = validate_preflight(plan, manifest, preflight_result)
                result['preflight_result_sha256'] = q.digest(preflight_result)
                stamps[Path(preflight_result)] = q.stamp(preflight_result)
            elif diagnostic and not coverage:
                require(scored_result is not None, 'diagnostics require the completed scored result')
                scored = q.read(scored_result)
                require(scored['status'] == 'complete' and not scored['diagnostic'] and not scored['failures']
                        and scored['manifest_sha256'] == result['manifest_sha256'], 'invalid scored diagnostic reference')
                result['scored_result_sha256'] = q.digest(scored_result)
                stamps[Path(scored_result)] = q.stamp(scored_result)
            from qualify_scc import HostMonitor, MacMonitor
            factory = monitor_factory or (HostMonitor if plan['platform'] == 'Linux' else MacMonitor)
            host = factory(output / 'host', max(plan['threads']))
            meta_by_cell, requests, snapshots, first_worker = {}, {}, {}, {}
            # Prepare all request bytes before native sampling; same plan/file budget.
            for item in plan['cases']:
                meta = q.metadata(plan, item, manifest)
                for setting in [s for s in settings(plan) if s['family'] == item['id']]:
                    cell, worker = setting['cell'], setting['workers']
                    directory = output / cell
                    directory.mkdir()
                    meta_by_cell[cell] = meta
                    result['cells'][cell] = dict(family=item['family'], workers=worker, fingerprints={}, worker_signatures={}, references={})
                    for arm in [*physical_arms(plan), *(['reference'] if coverage and diagnostic else [])]:
                        request = directory / (arm.replace('+', '_') + '.case')
                        mechanism = ('none', 'none') if arm == 'reference' else plan['arms'][arm]
                        prepare_request(q.resolve(manifest, item['input']), request, mechanism, worker)
                        requests[cell, arm] = request
                        stamps[request] = q.stamp(request)
            with deadline.observing(host):
                if historical is not None:
                    from ownership_panel3 import same_host
                    same_host(historical[0], host.report())
                if coverage and 'preflight_reuse' in plan:
                    require(host.report()['host']['hostname'].split('.')[0]==plan['preflight_reuse']['required_host'],
                            'amended coverage requires the original physical host')
                if reused:
                    from ownership_moments_repair import import_prefix
                    import_prefix(plan, manifest, output, result, host.report())
                    persist()
                for index, entry in enumerate(sequence[reused:], start=reused):
                    deadline.check()
                    host.check()
                    require(all(q.stamp(p) == v for p, v in stamps.items()), 'bound source/input changed during run')
                    cell, arm = entry['cell'], entry['arm']
                    mechanism = ('none', 'none') if arm == 'reference' else plan['arms'][arm]
                    meta = meta_by_cell[cell]
                    target = output / cell / f'call-{index:04d}.json'
                    log = target.with_suffix('.log')
                    traced = entry['phase'] != 'scored'
                    env = ['env', *[v for key in q.ENVIRONMENT for v in ('-u', key)], *[key + '=1' for key in q.THREAD_ENV]]
                    if traced:
                        env += ['FEREG_SOLVER_TRACE=1']
                    if diagnostic:
                        env += ['FEREG_PHASE_TIMING=1', 'FEREG_ACTION_MEMORY_AUDIT=1']
                    call = {**entry, 'status': 'started', 'input_sha256': q.digest(requests[cell, arm]),
                            'target': str(target.relative_to(output)), 'log': str(log.relative_to(output))}
                    result['calls'].append(call)
                    result['native_calls_started'] += 1
                    persist()
                    begin = time.monotonic_ns()
                    resources = execute([*env, str(q.resolve(manifest, plan['harness'])),
                                         str(q.resolve(manifest, plan['artifacts']['reference' if arm == 'reference' else 'candidate']['path'])), '--replay',
                                         str(requests[cell, arm]), str(target)], log, min(120, deadline.check()))
                    finish = time.monotonic_ns()
                    raw, columns, detail = q.read_native(target, log, meta)
                    require(detail['seconds'] <= 120, 'complete native timer exceeds call cap')
                    require(detail['solver_rows'][0][0] == q.cmgschur.EXPECTED_SOLVER['auto'], 'requested Auto identity changed')
                    detail.update(worker_signature=q.worker_signature(raw), model_signature=q.science.model_discrete_sha256(raw))
                    if traced:
                        evidence = q.science.resource_diagnostics(log.read_text(), None, False, {})
                        detail.update(evidence, solver_trace_lines=[line for line in log.read_text().splitlines() if line.startswith('FEREG_SOLVER_TRACE|')])
                        detail['cell_diagnostic'] = cell_diagnostic(log.read_text(), mechanism)
                        if mechanism[1] == 'deferred':
                            from ownership_panel3 import deferred_commitment
                            deferred_commitment(log.read_text())
                    if diagnostic:
                        if arm != 'reference':
                            detail['ownership_diagnostic'] = ownership_diagnostic(log.read_text(), mechanism)
                        detail['phase_lines'] = [line for line in log.read_text().splitlines()
                                                 if line.startswith(('FEREG_PHASE|', 'FEREG_OWNERSHIP|', 'FEREG_CELLS|'))]
                    if historical is not None and entry['phase'] == 'warmup':
                        from ownership_panel3 import compare_prior
                        compare_prior(historical[1], cell, arm, detail)
                    prior = snapshots.get((cell, arm))
                    if prior:
                        require(prior[2]['fingerprint'] == detail['fingerprint'], 'fixed-worker repeat changed')
                    elif diagnostic and not coverage:
                        require(detail['fingerprint'] == scored['cells'][cell]['fingerprints'][arm], 'diagnostic changes scored science')
                    elif prerequisite is not None:
                        require(detail['fingerprint'] == prerequisite['cells'][cell]['fingerprints'][arm],
                                'coverage timing changes preflight science')
                    call.update(detail, status='passed', oracle='passed', result_sha256=q.digest(target), log_sha256=q.digest(log),
                                columns_sha256=q.digest(columns), raw_result=raw, maxrss_bytes=resources['maxrss_bytes'],
                                started_monotonic_ns=begin, finished_monotonic_ns=finish)
                    if prior is None:
                        snapshots[cell, arm] = (raw, columns, detail)
                        record = result['cells'][cell]
                        record['fingerprints'][arm] = detail['fingerprint']
                        record['worker_signatures'][arm] = detail['worker_signature']
                        base = output / cell / ('reference-' + arm.replace('+', '_'))
                        record['references'][arm] = {'path': str(base.relative_to(output)), **q.retained_snapshot(base, raw, columns, meta)}
                        family = record['family']
                        if (family, arm) not in first_worker:
                            first_worker[family, arm] = (base, detail)
                        else:
                            before, before_detail = first_worker[family, arm]
                            q.compare_saved(before, base, meta)
                            require(before_detail['worker_signature'] == detail['worker_signature'], 'cross-worker discrete decisions changed')
                        arms = list(physical_arms(plan))
                        if arm != 'reference' and all((cell, a) in snapshots for a in arms):
                            compare_warmups(plan, {a: snapshots[cell, a] for a in arms}, meta)
                    deadline.check()
                    persist()
                    # Warmup columns stay until all scoped cross-arm checks have run.
                    if prior is not None:
                        columns.unlink()
                    elif arm == 'reference':
                        columns.unlink()
                    elif all((cell, a) in snapshots for a in physical_arms(plan)):
                        for a in physical_arms(plan):
                            snapshots[cell, a][1].unlink()
                require(result['native_calls_started'] == len(sequence), 'incomplete finite schedule')
            result['host'] = host.report()
            require(result['host']['isolation_ok'] and not result['host']['errors'], 'invalid host observation/isolation')
            result['host_observations_sha256'] = q.digest(output / 'host/host-observations.jsonl')
            require(all(q.stamp(p) == v for p, v in stamps.items()), 'final input/source drift')
            if coverage and diagnostic:
                from ownership_moments import execution_evidence
                result['execution_evidence'] = execution_evidence(plan, result)
                if reused:
                    result['native_calls_executed_here'] = result['native_calls_started'] - reused
            deadline.check()
            result['status'] = 'complete'
            persist()
            deadline.check()
    except BaseException as error:
        result['status'] = 'failed'
        result['failures'].append(dict(type=type(error).__name__, message=str(error)))
        if host is not None:
            result['host'] = host.report()
        persist()
        raise
    return result


def summaries(plan, calls):
    """Arithmetic paired differences; every family/worker setting has equal weight."""
    require(len(calls) == plan['call_cap'] and all(c['status'] == 'passed' for c in calls), 'incomplete/failed calls')
    require([{k: c[k] for k in ('cell', 'arm', 'phase', 'block', 'mirror')} for c in calls] == plan['schedule'], 'call order changed')
    result = []
    for cell in [setting['cell'] for setting in settings(plan)]:
        for role in plan['arms']:
            if role == 'B':
                continue
            alias = plan['aliases'][role]
            pairs = []
            for block in range(plan['blocks']):
                def mean(arm):
                    rows = [c['seconds'] for c in calls if c['cell'] == cell and c['arm'] == arm and c['phase'] == 'scored' and c['block'] == block]
                    require(len(rows) == 2 and all(math.isfinite(v) and v > 0 for v in rows), 'invalid paired complete-call timer')
                    return statistics.mean(rows)
                baseline, candidate = mean('B'), mean(alias)
                pairs.append(dict(block=block, baseline_s=baseline, candidate_s=candidate,
                                  difference_s=candidate-baseline, ratio=candidate/baseline,
                                  excess_s=candidate-baseline-max(.05*baseline, .001)))
            difference = statistics.mean(p['difference_s'] for p in pairs)
            result.append(dict(cell=cell, arm=role, paired_blocks=pairs, mean_difference_s=difference,
                               mean_ratio=statistics.mean(p['ratio'] for p in pairs),
                               mean_excess_s=statistics.mean(p['excess_s'] for p in pairs),
                               status='within_allowance_point_estimate' if statistics.mean(p['excess_s'] for p in pairs) <= 0 else 'adverse_point_estimate'))
    objectives = {role: statistics.mean(row['mean_difference_s'] for row in result if row['arm'] == role)
                  for role in plan['arms'] if role != 'B'}
    return result, objectives


def report(manifest, directory, output, *, accounting=None):
    manifest, directory, output = Path(manifest), Path(directory), Path(output)
    plan, result = q.read(manifest), q.read(directory / 'results.json')
    verify_files(plan, manifest)
    require(result['manifest_sha256'] == routing.identity(plan) and result['status'] == 'complete'
            and result['failures'] == [] and not result['diagnostic'] and result['retries'] == 0 and result['exclusions'] == []
            and result['elapsed_s'] < plan['budget_s'] and result['native_calls_started'] == plan['call_cap'], 'incomplete/failed panel')
    require(result['host']['isolation_ok'] and not result['host']['errors'], 'host evidence failed')
    require(q.digest(directory / 'host/host-observations.jsonl') == result['host_observations_sha256'], 'host observations changed')
    if plan['platform'] == 'Linux':
        require(accounting is not None, 'Linux report requires final accounting')
        q.accounting(Path(accounting).read_text(), result['host']['host'], normalize=True)
    historical = None
    if plan['panel'] == 3:
        from ownership_panel3 import prior_result, same_host
        historical = prior_result(plan, manifest)
        same_host(historical[0], result['host'])
    for call in result['calls']:
        target, log = directory / call['target'], directory / call['log']
        require(q.digest(target) == call['result_sha256'] and q.digest(log) == call['log_sha256'], 'raw call evidence changed')
        raw = q.read(target)
        require(raw.pop('columns') == [] and raw == call['raw_result'], 'raw result retention differs')
        timer = re.findall(r'^NATIVE_SECONDS=([0-9.]+)$', log.read_text(), re.M)
        require(len(timer) == 1 and float(timer[0]) == call['seconds'] and call['timing_field'] == 'NATIVE_SECONDS', 'scored timer differs')
        require(call['fingerprint'] == routing.identity({**q.exact_view(raw, False), 'columns_sha256': call['columns_sha256']})
                and call['worker_signature'] == q.worker_signature(raw)
                and call['fingerprint'] == result['cells'][call['cell']]['fingerprints'][call['arm']], 'own repeats/signatures differ')
        if call['phase'] == 'warmup':
            text = log.read_text()
            require(call['solver_trace_lines'] == [line for line in text.splitlines()
                                                   if line.startswith('FEREG_SOLVER_TRACE|')],
                    'warmup solver trace changed')
            evidence = q.science.resource_diagnostics(text, None, False, {})
            require(all(call[key] == value for key, value in evidence.items()), 'warmup resource/route evidence changed')
            require(call['cell_diagnostic'] == cell_diagnostic(text, plan['arms'][call['arm']]),
                    'warmup cell evidence changed')
            if plan['arms'][call['arm']][1] == 'deferred':
                from ownership_panel3 import deferred_commitment
                deferred_commitment(text)
            if historical is not None:
                from ownership_panel3 import compare_prior
                compare_prior(historical[1], call['cell'], call['arm'], call)
    for cell in result['cells'].values():
        for ref in cell['references'].values():
            base = directory / ref['path']
            require(q.digest(base.with_suffix('.json')) == ref['head'] and q.digest(base.with_suffix('.npz')) == ref['outputs'], 'retained arrays/head changed')
    if plan['panel'] == 3:
        from ownership_panel3 import compare_actions
        warm = {(call['cell'], call['arm']): call for call in result['calls'] if call['phase'] == 'warmup'}
        for cell in result['cells']:
            compare_actions(warm[cell, 'O+deferred'], warm[cell, 'O'])
    for item in plan['cases']:
        meta = q.metadata(plan, item, manifest)
        for arm in physical_arms(plan):
            matching = [s['cell'] for s in settings(plan) if s['family'] == item['id']]
            before = result['cells'][matching[0]]
            for cell in matching[1:]:
                after = result['cells'][cell]
                q.compare_saved(directory / before['references'][arm]['path'], directory / after['references'][arm]['path'], meta)
                require(before['worker_signatures'][arm] == after['worker_signatures'][arm], 'cross-worker decisions changed')
    comparisons, objectives = summaries(plan, result['calls'])
    value = dict(schema=1, kind=plan['kind'] + '-report', status='development_complete', activation_qualified=False,
                 panel=plan['panel'], native_calls=plan['call_cap'], result_sha256=q.digest(directory / 'results.json'),
                 manifest_sha256=routing.identity(plan), comparisons=comparisons,
                 equal_weight_mean_difference_s=objectives,
                 objective=f'Arithmetic mean of paired complete-command differences vs B; equal weight for all {len(settings(plan))} family/worker settings.',
                 limitations=['Six paired blocks on one host are not six independent hosts or an activation qualification.',
                              'All adverse observations are retained; this report does not select or launch another panel.'])
    if plan['kind'] == COVERAGE_KIND:
        from ownership_moments import uncertainty, validate_preflight
        preflight = directory.parent / plan['preflight_output'] / 'results.json'
        require(q.digest(preflight) == result['preflight_result_sha256'], 'preflight binding changed')
        validate_preflight(plan, manifest, preflight)
        value.update(uncertainty(plan, comparisons), execution_evidence=q.read(preflight)['execution_evidence'])
    save_new(output, value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('prepare'); p.add_argument('--generator', type=Path, required=True); p.add_argument('--output', type=Path, required=True)
    p = sub.add_parser('plan'); p.add_argument('config', type=Path); p.add_argument('output', type=Path)
    p = sub.add_parser('controls'); p.add_argument('config', type=Path); p.add_argument('output', type=Path)
    for name in ('run', 'diagnose'):
        p = sub.add_parser(name); p.add_argument('plan', type=Path); p.add_argument('output', type=Path)
        if name == 'diagnose': p.add_argument('--scored-result', type=Path, required=True)
    p = sub.add_parser('report'); p.add_argument('plan', type=Path); p.add_argument('directory', type=Path); p.add_argument('output', type=Path); p.add_argument('--accounting', type=Path)
    args = parser.parse_args()
    if args.command == 'prepare': value = prepare(args.generator, args.output)
    elif args.command == 'plan': value = freeze(args.config, args.output)
    elif args.command == 'controls': value = controls(args.config, args.output)
    elif args.command in ('run', 'diagnose'): value = run(args.plan, args.output, diagnostic=args.command == 'diagnose', scored_result=getattr(args, 'scored_result', None))
    else: value = report(args.plan, args.directory, args.output, accounting=args.accounting)
    print(json.dumps({'status': value.get('status', 'prepared'), 'kind': value['kind']}))


if __name__ == '__main__':
    main()
