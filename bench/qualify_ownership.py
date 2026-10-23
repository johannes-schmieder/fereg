"""Prospective ownership/cell qualification; historical routing plans are unchanged.

This module frames experiment tokens separately from RoutingPreset. It neither
selects a finalist nor generates inputs/references or executes diagnostic calls.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import shutil

import routing
import routing_cases
import routing_science as science
import ownership_qualification_cases as added_cases
import routing_oracle
import numpy as np
from native_io import case_header, digest, validate_result
from replay_plugins import allowed_difference, differences, exact_view

MODE = 'ownership-cells-v1'
CELL_REQUESTS = ('cells_eager_both', 'cells_deferred_both')
REQUESTS = ('ownership_both', *CELL_REQUESTS)
ORIGINAL = 'f35f3a4dbb306bd4c4f5f7d46a406be892bd0749'
INCUMBENT = 'c16c0b5555338d8b20b184e6e51ad2acbe6a4e6e'
BASELINE_PLAN_SHA256 = {
    'Darwin': '04a734e50c93feba47d484479b9200f843bdd70a2857a08f9a921c1079eb6a7d',
    'Linux': 'a67f6d654f7728a66e1c1730a07c35b145b447f0df20b5527f9ca86c53f3d601',
}
ADDED = ('ownership-nonmovers', 'ownership-hubs', 'ownership-inactive-zero',
         'ownership-cache-below', 'ownership-cache-at', 'ownership-cache-above',
         'ownership-repeated-large', 'ownership-weak-large')
ROWS = dict(zip(ADDED, (20000, 64000, 20000, 262143, 262144, 262145, 600000, 480000)))
TRANSPORT_SOURCES = {
    'crates/fereg-plugin/src/ffi.rs': '0a8955b003850ff3e3d8166ee2ef1338fd010adcc8d27341b76a5dfd5005aade',
    'crates/fereg-plugin/src/metadata_memory.rs': 'a48cd6c59157ebc2a845a4d9786a5ea4c80a553b0b05b11bf1b55b6e6b4520fd',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def enabled(manifest):
    if 'qualification' not in manifest:
        return False
    require(manifest['qualification'] == MODE, 'unknown prospective qualification mode')
    return True


def validate(manifest):
    import qualify as q
    require(enabled(manifest) and 'diagnostic' not in manifest, 'ownership is a qualification, not a diagnostic')
    require(not any(k in manifest for k in ('continuation', 'recovery')), 'ownership budget cannot restart or add observations')
    require(manifest['coverage'] == 'ownership' and manifest['threads'] == [1, 4]
            and manifest['seed'] == 2026106306 and manifest['timeout_s'] == 120,
            'ownership coverage/worker/order/call budget changed')
    linux = manifest['platform'] == 'Linux'
    require(manifest['platform'] in ('Linux', 'Darwin')
            and manifest['blocks'] == (2 if linux else 24)
            and manifest['replicas'] == (12 if linux else 1)
            and manifest['shards'] == (3 if linux else 1)
            and manifest['budget_s'] == (3600 if linux else 14400)
            and (manifest.get('min_distinct_hosts_per_cell') == 12 if linux
                 else 'min_distinct_hosts_per_cell' not in manifest), 'ownership fixed platform budget changed')
    cases = manifest['cases']
    require(len(cases) == 29 and len({c['id'] for c in cases}) == 29
            and {c['family'] for c in cases} == q.FAMILIES | set(ADDED), 'ownership requires original 42 plus added 16 cells')
    require(set(manifest['artifacts']) == {'original', 'incumbent', 'candidate'}, 'ownership timing requires exactly three arms')
    original, incumbent, candidate = (manifest['artifacts'][r] for r in ('original', 'incumbent', 'candidate'))
    require(original['revision'] == ORIGINAL and original['request'] == 'auto' and original['families'] == [],
            'original public reference changed')
    require(incumbent['revision'] == INCUMBENT and incumbent['request'] == 'auto'
            and incumbent['families'] == ['many', 'two'], 'accepted c16 GATES reference changed')
    require(candidate['request'] in REQUESTS and candidate['families'] == ['many', 'two']
            and re.fullmatch('[0-9a-f]{40}', candidate['revision']) is not None
            and manifest['scope'] == ('exact' if candidate['request'] == REQUESTS[0] else 'two'),
            'candidate request/scope changed')
    require(set(manifest['ownership_binding']) == {'baseline_record',
            *(role+'_'+kind for role in ('original', 'incumbent', 'candidate') for kind in ('source', 'build'))},
            'ownership source/original-input bindings missing')
    if 'files' in manifest:
        require(all(routing.sha(manifest['files'].get(p)) for p in manifest['ownership_binding'].values()),
                'ownership provenance not bound')
        identities = {(manifest['files'][a['path']], a['request']) for a in manifest['artifacts'].values()}
        require(len(identities) == 3, 'three timed arms cannot alias')
    require(manifest.get('warmups', 1) == 1, 'ownership warmup changed')


def _bound_hash(manifest, path):
    require(path in manifest['files'] and routing.sha(manifest['files'][path]), 'missing frozen file identity')
    return manifest['files'][path]


def validate_bindings(manifest, manifest_path):
    """Check original identities and source contracts; no historical path walk."""
    import qualify as q
    validate(manifest)
    binding = manifest['ownership_binding']
    record = q.read(q.resolve(manifest_path, binding['baseline_record']))
    sealed = record['plans'][manifest['platform']]
    baseline = sealed['content']
    encoded = (json.dumps(baseline, sort_keys=True, indent=2, allow_nan=False)+'\n').encode()
    require(hashlib.sha256(encoded).hexdigest() == sealed['sha256'] == BASELINE_PLAN_SHA256[manifest['platform']], 'embedded original plan bytes changed')
    require(len(baseline['cases']) == 21 and {c['family'] for c in baseline['cases']} == q.FAMILIES
            and baseline['coverage'] == 'routing', 'wrong original 42-cell plan')
    current = {c['family']: c for c in manifest['cases']}
    for before in baseline['cases']:
        after = current[before['family']]
        require(after['id'] == before['id'] and after['memory_limit_bytes'] == before['memory_limit_bytes']
                and after['shape'] == before['shape'] and after['oracle']['kind'] == before['oracle']['kind'],
                'original case/model declaration changed')
        for key in ('input', 'metadata'):
            require((before[key] is None and after[key] is None)
                    or (before[key] is not None and after[key] is not None
                        and _bound_hash(manifest, after[key]) == baseline['files'][before[key]]),
                    'original input/metadata bytes changed')
        require(set(after['oracle']) == set(before['oracle']), 'original reference shape changed')
        for key in ('path', 'arrays'):
            if key in before['oracle']:
                require(_bound_hash(manifest, after['oracle'][key]) == baseline['files'][before['oracle'][key]],
                        'original reference bytes changed')
    for role in ('original', 'incumbent', 'candidate'):
        source = q.read(q.resolve(manifest_path, binding[role+'_source']))
        build = q.read(q.resolve(manifest_path, binding[role+'_build']))
        artifact = manifest['artifacts'][role]
        files = source['files']
        require(source['fereg'] == build['fereg'] == artifact['revision']
                and source['cmg'] == build['cmg']
                and re.fullmatch('[0-9a-f]{40}', source['cmg']) is not None
                and routing.sha(source['archive_sha256'])
                and isinstance(files, dict) and bool(files)
                and all(isinstance(p, str) and bool(p) and not Path(p).is_absolute()
                        and '..' not in Path(p).parts and routing.sha(h) for p, h in files.items()),
                'actual source/build revision or source map changed')
        require(build['plugin_sha256'] == _bound_hash(manifest, artifact['path'])
                and build['source_hashes_verified_before_after'] is True
                and build['native_abi_load_panic'] == 'passed'
                and isinstance(build['build'], str) and bool(build['build'].strip())
                and build['build'] == artifact['build'], 'actual artifact build/ABI evidence differs')
        if role != 'original':
            require(all(files.get(p) == h for p, h in TRANSPORT_SOURCES.items()),
                    'actual source/64-bit argument-parent contract changed')
    return baseline


def validate_inputs(manifest, manifest_path):
    """Prospective held-out shape/reference checks, performed once before freezing."""
    import qualify as q
    validate_bindings(manifest, manifest_path)
    bodies = set()
    by_family = {c['family']: c for c in manifest['cases']}
    for item in manifest['cases']:
        source = q.resolve(manifest_path, item['input'])
        inspected = routing_cases.inspect_case(source, item['family'])
        require(inspected['body_sha256'] not in bodies, 'qualification bodies collide')
        bodies.add(inspected['body_sha256'])
        if item['family'] not in ADDED:
            continue
        require(item['id'] == item['family'] and item['metadata'] is not None
                and item['memory_limit_bytes'] is None, 'added case identity/metadata changed')
        meta = q.read(q.resolve(manifest_path, item['metadata']))
        require(meta['seed'] == 2026106302 and meta['family'] == item['family']
                and meta['case_sha256'] == _bound_hash(manifest, item['input'])
                and meta['body_sha256'] == inspected['body_sha256'] and not meta.get('small', False),
                'added held-out input provenance changed')
        with source.open('rb') as stream:
            shape, argv = case_header(stream)
        fields = options(argv)
        require(shape[0] == ROWS[item['family']] and fields.get('candidate_all') == '1'
                and fields.get('input_complete') == '0' and fields.get('memory') == 'fast'
                and fields.get('vce') == 'robust' and fields.get('solver') == fields.get('preconditioner') == 'auto'
                and inspected['absorbed_terms'] == 2 and inspected['categorical_intercepts'],
                'added shape/model/eligibility hints changed')
        truth = q.metadata(manifest, item, manifest_path)
        validate_added(item, meta, routing_oracle.read_case(source), truth)
    boundary = [q.resolve(manifest_path, by_family[f'ownership-cache-{suffix}']['input'])
                for suffix in ('below', 'at', 'above')]
    validate_boundary(boundary)


def validate_added(item, metadata, parsed, truth):
    """Bind actual canonical columns and loaded independent truth to each shape.

    read_case validates candidate bits, finite values and canonical source/model
    matrices. metadata() loads and validates the full independent rank certificate
    (or the analytic inactive witness); labels alone establish neither contract.
    """
    spec = added_cases.specification(item['family'])
    require(all(metadata.get(k) == v for k, v in added_cases.specification_metadata(spec).items()),
            'added seed/streams/transform/dimensions metadata differs')
    require(parsed['rows'] == spec['rows'] and parsed['columns'] == spec['regressors']+10
            and parsed['x'].shape == (spec['rows'], spec['regressors'])
            and len(parsed['y']) == spec['rows'] and len(parsed['factors']) == 2
            and all(np.asarray(f).shape == (spec['rows'],) for f in parsed['factors'])
            and [len(np.unique(f)) for f in parsed['factors']] == [spec['workers'], spec['firms']]
            and parsed['weights'].shape == (spec['rows'],) and np.all(parsed['weights'] == 1)
            and parsed['include_intercept'] == spec['include_intercept']
            and parsed['vce'] == 'robust' and not parsed['account_cluster_nesting']
            and not parsed['keep_singletons'], 'added actual width/levels/unit-weight model differs')
    if item['family'] == 'ownership-inactive-zero':
        require(item['oracle']['kind'] == 'inactive', 'inactive control requires canonical independent witness')
        reference = truth['oracle']
        require(reference['regressors'] == spec['regressors']
                and len(reference['fitted']) == len(reference['residual']) == spec['rows'],
                'inactive independent witness dimensions differ')
    else:
        require(item['oracle']['kind'] == 'routing', 'added case needs input-bound independent reference')
        reference = truth['routing_oracle']
        require(reference['rows'] == reference['retained_rows'] == spec['rows']
                and reference['regressors'] == spec['regressors']
                and reference['include_intercept'] == spec['include_intercept']
                and reference.get('vce', 'robust') == 'robust'
                and reference['certificate']['levels'] == [spec['workers'], spec['firms']]
                and reference['certificate'].get('memory_budget_bytes', 1 << 30)
                    == spec['oracle_memory_budget_bytes'],
                'added independent rows/rank/model/oracle allowance differs')


def validate_boundary(paths):
    """Exact shared numeric rows; no inference from a boundary label."""
    from contextlib import ExitStack
    with ExitStack() as stack:
        streams = [stack.enter_context(Path(p).open('rb')) for p in paths]
        headers = [case_header(s) for s in streams]
        shapes, args = zip(*headers)
        require([s[0] for s in shapes] == [262143, 262144, 262145]
                and all(s[1:] == shapes[0][1:] for s in shapes)
                and args[0] == args[1] == args[2], 'boundary framing/model changed')
        for _ in range(shapes[0][1]):
            require(all(s.readline() == b'0\n' for s in streams), 'boundary source is not numeric')
            before, center, after = [s.read(shape[0]*17) for s, shape in zip(streams, shapes)]
            require(len(center) == 262144*17 and len(before) == 262143*17 and len(after) == 262145*17
                    and before == center[:-17] and after == center+center[-17:], 'boundary rows were regenerated or substituted')
        # Matrix and macro definitions must be identical too.
        while True:
            chunks = [s.read(1 << 20) for s in streams]
            require(chunks[0] == chunks[1] == chunks[2], 'boundary metadata body differs')
            if not chunks[0]:
                break


def options(argv):
    require(argv and argv[0] == b'estimate-ols', 'ownership requires OLS')
    pairs = [a.decode('utf-8').split('=', 1) for a in argv[1:]]
    require(all(len(p) == 2 for p in pairs) and len({p[0] for p in pairs}) == len(pairs), 'duplicate or malformed request options')
    return dict(pairs)


def arguments(source_args, request, workers):
    fields = options(source_args)
    require('inject' not in fields and 'break_marker_token' not in fields, 'input already has developer instrumentation')
    require(request in ('auto', *REQUESTS) and type(workers) is int and workers in (1, 4), 'unknown ownership invocation')
    updates = {'solver': 'auto', 'preconditioner': 'auto', 'threads': str(workers)}
    removed = {*updates, 'cmgdegree', 'cmgsamples', 'cmgseed'}
    argv = [a for a in source_args if a.split(b'=', 1)[0].decode() not in removed]
    argv += [f'{key}={value}'.encode() for key, value in updates.items()]
    if request != 'auto':
        argv.append(('inject='+request).encode())
    return argv


def prepare(source, destination, request, workers):
    with Path(source).open('rb') as stream, Path(destination).open('xb') as out:
        shape, argv = case_header(stream)
        argv = arguments(argv, request, workers)
        shape[2] = len(argv)
        out.write(('FEREG_REPLAY_V1\n'+' '.join(map(str, shape))+'\n').encode())
        for arg in argv:
            out.write(str(len(arg)).encode()+b'\n'+arg+b'\n')
        shutil.copyfileobj(stream, out, 1 << 20)
    return argv


def argument_parent(argv):
    """Actual bound 64-bit FFI argv: String slots plus exactly reserved payloads."""
    options(argv)
    return 24*len(argv)+32+sum(len(a) for a in argv[1:])


def transport_evidence(argv, source_args, request, workers, actions):
    require(argv == arguments(source_args, request, workers), 'executed ownership argv differs')
    public = arguments(source_args, 'auto', workers)
    parent, public_parent = argument_parent(argv), argument_parent(public)
    delta = parent-public_parent
    require(delta == (0 if request == 'auto' else 24+len(('inject='+request).encode())), 'unexpected transport delta')
    normalized = []
    for action in actions:
        used, limit = action['memory_bytes_used'], action['memory_bytes_limit']
        require(type(used) is type(limit) is int and parent <= used <= limit,
                'raw memory cap failed or argument parent is not represented')
        require(0 <= action['iterations_used'] <= action['iterations_limit'], 'raw iteration cap failed')
        normalized.append({k: v-delta if k == 'memory_bytes_used' else v
                           for k, v in action.items() if k != 'memory_bytes_limit'})
    return dict(argv=[a.decode() for a in argv], argument_parent_bytes=parent,
                public_argument_parent_bytes=public_parent, argument_parent_delta_bytes=delta,
                normalized_action_resources=normalized,
                basis='Bound 64-bit FFI exact argv/String reservations; raw caps checked before normalization. Real ownership storage changes are retained.')


def resource_policy(manifest, role, meta):
    if role == 'original':
        return None
    if routing.incremental_eligible(meta, 'two'):
        return routing.GATES
    if routing.incremental_eligible(meta, 'many'):
        return 'incremental-many'
    return None


def resource_detail(manifest, role, meta, log, argv, source_args, workers):
    import two_fe_policy_development as gates
    policy = resource_policy(manifest, role, meta)
    detail = science.resource_diagnostics(log, policy, False, {})
    detail['gate_events'] = gates.gate_events(log) if policy == routing.GATES else []
    detail['solver_trace_lines'] = [line for line in log.splitlines() if line.startswith('FEREG_SOLVER_TRACE|')]
    detail['ownership_transport'] = transport_evidence(argv, source_args, manifest['artifacts'][role]['request'],
                                                       workers, detail['action_resources'])
    if (manifest['artifacts'][role]['request'] == 'cells_deferred_both'
            and routing.incremental_eligible(meta, 'two')):
        from ownership_panel3 import deferred_commitment
        detail['deferred_commitment'] = deferred_commitment(log)
    return detail


def installed_hierarchies(detail):
    require(len(detail['route_trace']) == len(detail['route_hierarchy']), 'hierarchy/route count differs')
    return [h for r, h in zip(detail['route_trace'], detail['route_hierarchy']) if r.get('installed') == '1']


def compare_artifacts(manifest, meta, snapshots):
    import qualify as q
    candidate, incumbent, original = (snapshots[r] for r in ('candidate', 'incumbent', 'original'))
    cells = manifest['artifacts']['candidate']['request'] in CELL_REQUESTS and routing.incremental_eligible(meta, 'two')
    if cells and manifest['artifacts']['candidate']['request'] == 'cells_deferred_both':
        from ownership_panel3 import compare_actions
        compare_actions(candidate[2], incumbent[2])
    if not cells:
        require(candidate[2]['fingerprint'] == incumbent[2]['fingerprint'], 'ownership/unaffected science changed relative to c16')
        for key in ('route_trace', 'route_hierarchy', 'gate_events'):
            require(candidate[2][key] == incumbent[2][key], 'ownership changed routing decisions')
    else:
        science.compatibility(incumbent[0], candidate[0], incumbent[1], candidate[1], meta, 'auto')
        a, b = installed_hierarchies(incumbent[2]), installed_hierarchies(candidate[2])
        if a and b:
            require(a == b, 'cell operator changed original CMG hierarchy when both built')
    if q.affected(meta, ['many', 'two']):
        science.compatibility(original[0], candidate[0], original[1], candidate[1], meta, 'auto',
                              selected_policy=routing.policy(routing.PORTFOLIO_UNSEEDED))
    else:
        require(original[2]['fingerprint'] == candidate[2]['fingerprint'], 'unaffected original-public science changed')


def verify_call(manifest, item, manifest_path, directory, call, workers, verify_file, *, traced):
    """Reparse retained call bytes; no native execution or replacement sample."""
    import qualify as q
    target, log = (directory/call[k] for k in ('target', 'log'))
    require(target.resolve().is_relative_to(directory.resolve()) and log.resolve().is_relative_to(directory.resolve()),
            'call evidence outside result directory')
    verify_file(target, call['result_sha256'])
    verify_file(log, call['log_sha256'])
    raw = q.read(target)
    require(raw.pop('columns') == [] and raw == call['raw_result'], 'retained raw result differs')
    validate_result(raw)
    text = log.read_text()
    timers = re.findall(r'^NATIVE_SECONDS=([0-9.]+)$', text, re.M)
    require(len(timers) == 1 and float(timers[0]) == call['seconds'] and 0 < call['seconds'] <= 120
            and call['timing_field'] == 'NATIVE_SECONDS', 'complete-call timer changed')
    require(call['fingerprint'] == routing.identity({**exact_view(raw, False), 'columns_sha256': call['columns_sha256']}),
            'retained fingerprint differs')
    require(('ownership_transport' in call) == traced, 'missing/unexpected warmup resource evidence')
    if traced:
        with q.resolve(manifest_path, item['input']).open('rb') as stream:
            _, source_args = case_header(stream)
        argv = [a.encode() for a in call['ownership_transport']['argv']]
        detail = resource_detail(manifest, call['execution'], routing_cases.inspect_case(q.resolve(manifest_path, item['input'])),
                                 text, argv, source_args, workers)
        require(all(call[k] == v for k, v in detail.items()), 'retained resource/argv evidence differs')
    return raw


def verify_report_cell(manifest, item, manifest_path, directory, cell, verify_file):
    import qualify as q
    workers = int(cell['id'].rsplit('-t', 1)[1])
    warm = {c['execution']: c for c in cell['warmups']}
    require(set(warm) == {'original', 'incumbent', 'candidate'}, 'ownership warmup roles missing')
    heads = {}
    for call in cell['warmups']:
        heads[call['execution']] = verify_call(manifest, item, manifest_path, directory, call, workers,
                                               verify_file, traced=True)
    for block in cell['blocks']:
        for call in block['calls']:
            verify_call(manifest, item, manifest_path, directory, call, workers, verify_file, traced=False)
    meta = routing_cases.inspect_case(q.resolve(manifest_path, item['input']), item['family'])
    candidate = warm['candidate']
    cells = manifest['artifacts']['candidate']['request'] in CELL_REQUESTS and routing.incremental_eligible(meta, 'two')
    if cells and manifest['artifacts']['candidate']['request'] == 'cells_deferred_both':
        from ownership_panel3 import compare_actions
        compare_actions(candidate, warm['incumbent'])
    for role in ('incumbent', 'original'):
        exact = (not cells if role == 'incumbent' else not q.affected(meta, ['many', 'two']))
        if exact:
            require(candidate['fingerprint'] == warm[role]['fingerprint'], 'retained unaffected/ownership science differs')
        else:
            q.compare_saved(directory/cell['references'][role], directory/cell['references']['candidate'], meta)
            scope = (science.replay_scope(heads['candidate'], 'auto', meta) if role == 'incumbent' else
                     science.portfolio_scope(heads[role], heads['candidate'], meta, 'auto',
                                             routing.policy(routing.PORTFOLIO_UNSEEDED)))
            changed = differences(exact_view(heads[role], False), exact_view(heads['candidate'], False))
            require(not [p for p in changed if not allowed_difference(p, scope)], 'retained comparison changed undeclared fields')
            require(science.model_discrete_sha256(heads[role]) == science.model_discrete_sha256(heads['candidate']),
                    'retained comparison changed model decisions')
    if not cells:
        for key in ('route_trace', 'route_hierarchy', 'gate_events'):
            require(candidate[key] == warm['incumbent'][key], 'retained ownership routing decisions differ')
    else:
        a, b = installed_hierarchies(warm['incumbent']), installed_hierarchies(candidate)
        if a and b:
            require(a == b, 'retained original hierarchies differ')


def diagnostic_proposal(manifest):
    """A future separately frozen prerequisite, never executed by qualify.run."""
    validate(manifest)
    schedule = []
    for family in ADDED:
        for workers in (1, 4):
            for arm, repeats in (('incumbent', 1), ('uninjected_candidate', 2), ('candidate', 2)):
                schedule += [dict(cell=f'{family}-t{workers}', family=family, workers=workers,
                                  arm=arm, repeat=repeat) for repeat in range(repeats)]
    return dict(kind=MODE+'-diagnostic-proposal', native_calls=80, timeout_s=120, budget_s=2400,
                retries=0, performance_selection=False, requires_separate_frozen_execution=True,
                schedule=schedule,
                purpose='Attest actual moment construction/omission, retained-score work, exact/scoped science and raw caps before qualification; never replace a timed observation.')
