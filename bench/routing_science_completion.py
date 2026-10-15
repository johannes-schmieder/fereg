"""Eight missing scientific repeats after a reviewed comparator-only correction.

Original contexts, observations and proofs remain immutable. This module reads
evidence and derives admission; it never dispatches native calls or reads C.
"""
from __future__ import annotations

import copy
import hashlib
import json
import math
from pathlib import Path
import re
import struct

import cmgschur
import routing
import routing_blocks as blocks
import routing_run as runner
from native_io import diagnostics, digest, validate_result
from replay_plugins import exact_view

KIND = 'routing_science_completion_v1'
SOURCES = {'routing_run.py', 'routing_blocks.py', 'time_blocks.py'}
ERROR = ('scoped replay changed an unaffected field: '
         '/matrices/RSOLVER/bits/8; /matrices/RSOLVER/bits/9')
INPUTS = {'old_context', 'old_declaration', 'old_records', 'new_context',
          'new_declaration', 'diagnostic_review', 'transition_review', 'archives', 'execution'}
require = blocks.require


class VerificationPass:
    """Reuse hashes only within one validation, while file identity is stable."""
    def __init__(self):
        self.verified = {}

    @staticmethod
    def stamp(path):
        value = path.stat()
        return value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns

    def digest(self, source):
        path = Path(source).resolve(strict=True)
        before = self.stamp(path)
        previous = self.verified.get(path)
        if previous is not None:
            require(previous[0] == before, 'evidence changed within one validation pass')
            return previous[1]
        sha = digest(path)
        require(self.stamp(path) == before, 'evidence changed while its hash was checked')
        self.verified[path] = before, sha
        return sha

    def check(self, bound):
        require(isinstance(bound, dict) and set(bound) == {'path', 'sha256'}
                and routing.sha(bound['sha256']), 'invalid completion evidence binding')
        path = Path(bound['path']).resolve(strict=True)
        require(str(path) == bound['path'] and self.digest(path) == bound['sha256'], 'completion evidence identity changed')
        return path


def _references(verified):
    return [{'path': str(path), 'sha256': sha} for path, sha in verified.items()]


def current_sources():
    import time_blocks
    import routing_block_deployment
    paths = [*runner.tool_identities(), Path(__file__), Path(blocks.__file__),
             Path(time_blocks.__file__), Path(routing_block_deployment.__file__)]
    return [blocks.reference(path) for path in sorted(set(paths))]


def _resolver(archives, dependencies):
    require(set(archives) == SOURCES, 'completion needs exactly three original source archives')
    aliases, verifier = {}, VerificationPass()
    for name, bound in archives.items():
        original = str(Path(__file__).with_name(name).resolve())
        archived = verifier.check(bound)
        require(str(archived) != original, 'original source must be retained separately')
        aliases[(original, bound['sha256'])] = archived
        dependencies.append(bound)

    def resolve(bound):
        path = aliases.get((str(Path(bound['path']).resolve()), bound['sha256']))
        if path is None:
            path = verifier.check(bound)
        require(verifier.digest(path) == bound['sha256'], 'historical source binding changed')
        dependencies.append({'path': str(path), 'sha256': bound['sha256']})
        return path
    resolve.verifier = verifier
    return resolve


def _rows(records, key, cid):
    return [row for row in records[key] if row['cell'] == cid]


def _proof(witness, cell, manifest, arms, old, resolve):
    path = blocks.checked(witness)
    value, dependencies = runner.evidence(path, cell, manifest, arms, stage='B',
        selection_sha256=old['selection_sha256'], expected_tools=old['tool_sha256'],
        dependency_resolver=resolve,
        dependency_verifier=resolve.verifier.digest if hasattr(resolve, 'verifier') else None)
    proof = routing.unseal(routing.read_json((path.parent / value['scoped_replay']['path']).resolve()))
    require(proof['prerequisites'] == _rows(old, 'prerequisites', cell['id'])
            and proof['repeat_checks'] == _rows(old, 'records', cell['id']),
            'inherited proof does not reproduce original observations')
    return value, proof, _references(dependencies)


def _state(inputs):
    import routing_block_deployment as deployment
    require(set(inputs) == INPUTS, 'incomplete science completion inputs')
    dependencies = [inputs[key] for key in INPUTS - {'archives', 'execution'}]
    resolve = _resolver(inputs['archives'], dependencies)
    context, manifest = blocks.load_context(inputs['new_context'])
    old_context = blocks.read(inputs['old_context'], sealed=True)
    require(context['kind'] == blocks.KIND and context['stage'] == 'B'
            and old_context.get('coordinator_sha256') == inputs['archives']['routing_blocks.py']['sha256']
            and {k: v for k, v in context.items() if k != 'coordinator_sha256'} ==
                {k: v for k, v in old_context.items() if k != 'coordinator_sha256'},
            'completion changed original B design or scientific identity')
    before = blocks.read(inputs['old_declaration'], sealed=True)
    after = blocks.read(inputs['new_declaration'], sealed=True)
    require(before['b_context'] == inputs['old_context'] and after['b_context'] == inputs['new_context']
            and after == deployment.declare(inputs['new_context'], after['design'])
            and {k: v for k, v in before.items() if k not in {'b_context', 'sources'}} ==
                {k: v for k, v in after.items() if k not in {'b_context', 'sources'}},
            'completion changed original prospective C contract')
    original_sources = []
    for bound in after['sources']:
        if bound['path'] == str(Path(__file__).resolve()):
            continue
        original = copy.deepcopy(bound)
        name = Path(bound['path']).name
        if name in SOURCES and bound['path'] == str(Path(__file__).with_name(name).resolve()):
            original['sha256'] = inputs['archives'][name]['sha256']
        original_sources.append(original)
    require(before['sources'] == original_sources, 'original C source inventory changed')
    for bound in before['sources']:
        resolve(bound)
    dependencies += after['sources']
    old = blocks.read(inputs['old_records'])
    tools = {path.name: sha for path, sha in runner.tool_identities().items()}
    tools['routing_run.py'] = inputs['archives']['routing_run.py']['sha256']
    arms = set().union(*(set(blocks.required_arms(context, c)) for c in context['cells']))
    require(old.get('status') == 'complete_with_failed_cells'
            and blocks.mac_platform(old.get('platform')) and old.get('purpose') == 'scientific_prerequisites'
            and old.get('qualification_context') == inputs['old_context'] and old.get('stage') == 'B'
            and old.get('selection_sha256') == routing.identity(old_context)
            and old.get('manifest_sha256') == routing.identity(manifest)
            and old.get('harness_sha256') == manifest['harness_sha256']
            and old.get('artifacts') == {a: manifest['arms'][a]['sha256'] for a in arms}
            and old.get('cases') == {c['id']: c['case_sha256'] for c in context['cells']}
            and old.get('tool_sha256') == tools and old.get('warmups') == 0 and old.get('repetitions') == 1
            and old.get('timeout_s') == 120 == manifest['failure_policy']['per_call_timeout_s']
            and old.get('budget_s') == manifest['failure_policy']['aggregate_budget_s']
            and type(old.get('elapsed_s')) in {int, float} and math.isfinite(old['elapsed_s'])
            and 0 <= old['elapsed_s'] <= old['budget_s'] - 1200,
            'original scientific cohort or remaining budget changed')
    for family, checker in [('cmg', runner.cmg_worker_checks), ('generic_it', runner.generic_it_worker_checks)]:
        checks, blocked = checker(old)
        require(not blocked and old.get(family + '_worker_checks') == checks,
                'original worker diagnostic checks changed')
    review = blocks.read(inputs['diagnostic_review'])
    require(review.get('kind') == 'independent_routing_scientific_checker_failure_review'
            and review.get('status') == 'passed' and review.get('original_records') == inputs['old_records']
            and review.get('original_tool_sha256') == tools and review.get('raw_evidence'),
            'independent original failure review missing')
    for bound in review['raw_evidence']:
        resolve(bound)
    transition = blocks.read(inputs['transition_review'])
    expected = {key: inputs[key] for key in ['old_records', 'old_context', 'new_context',
                                            'old_declaration', 'new_declaration', 'archives']}
    require(transition.get('kind') == KIND + '_source_review' and transition.get('status') == 'passed'
            and all(transition.get(k) == v for k, v in expected.items())
            and transition.get('current_sources') == current_sources() and transition.get('evidence')
            and isinstance(transition.get('executor'), dict)
            and transition['executor'] in transition['evidence'],
            'reviewed current source transition missing')
    dependencies += [*transition['current_sources'], *transition['evidence']]
    for bound in dependencies:
        resolve.verifier.check(bound)
    return context, manifest, old, resolve, dependencies


def declare(inputs):
    """Validate metadata and hashes, then freeze precisely the eight absent calls."""
    context, manifest, old, resolve, dependencies = _state(inputs)
    first = {(r['cell'], r['arm']): r for r in old['prerequisites']}
    repeats = {(r['cell'], r['arm']): r for r in old['records']}
    expected = {(c['id'], a) for c in context['cells'] for a in blocks.required_arms(context, c)}
    require(len(first) == len(old['prerequisites']) and len(repeats) == len(old['records'])
            and set(first) == set(repeats) == expected, 'original cohort has incomplete or duplicate requests')
    missing = [key for key in first if first[key]['status'] == 'blocked']
    affected = {cid for cid, _ in missing}
    require(len(missing) == 8 and len(affected) == 2 and all(
        {a for cid, a in missing if cid == cell} == {'lsmr', 'map', 'it', 'it-coefficients'}
        for cell in affected), 'completion is limited to the eight reviewed missing repeats')
    execution = inputs['execution']
    require(set(execution) == {'harness', 'cells'} and set(execution['cells']) == affected
            and execution['harness']['sha256'] == manifest['harness_sha256'], 'completion execution coverage changed')
    dependencies.append(execution['harness'])
    requests = []
    for cell in context['cells']:
        cid, required = cell['id'], blocks.required_arms(context, cell)
        outcomes = runner.prerequisite_outcomes(manifest, required, _rows(old, 'prerequisites', cid), _rows(old, 'records', cid))
        if cid not in affected:
            require(runner.proof_admissible(manifest, outcomes), 'unrelated original prerequisite failed')
            ref = old['qualified_wrappers'][cid]
            witness = {'path': str(blocks.checked(inputs['old_records']).parent / ref['path']), 'sha256': ref['sha256']}
            _, _, proof_deps = _proof(witness, cell, manifest, required, old, resolve)
            dependencies += proof_deps
            continue
        data = execution['cells'][cid]
        require(set(data) == {'source', 'witness', 'arms'} and set(data['arms']) == {a for c, a in missing if c == cid}
                and data['source']['sha256'] == cell['case_sha256'], 'affected input coverage changed')
        dependencies += [data['source'], data['witness']]
        _, frozen = runner.evidence(blocks.checked(data['witness']), cell, manifest, required,
            prerequisites_only=True, stage='B', dependency_resolver=resolve,
            dependency_verifier=resolve.verifier.digest if hasattr(resolve, 'verifier') else None)
        dependencies += _references(frozen)
        for arm in required:
            key = cid, arm
            if key not in missing:
                require(outcomes[arm]['status'] in {'passed', 'unavailable'}, 'unrelated failed arm cannot be repaired')
                continue
            row, repeat = first[key], repeats[key]
            require(row.get('error') == ERROR and row.get('process_completed') is True
                    and row.get('dispatched') is True and row.get('call_limit_s') == 120
                    and row.get('failure', {}).get('kind') == 'science_or_resource_failed'
                    and repeat.get('status') == 'not_run' and repeat.get('repetition') == 0
                    and repeat.get('blocked_by_sha256') == routing.observation_identity(row),
                    'original failure is not the reviewed comparison-only interruption')
            invocation = data['arms'][arm]
            require(set(invocation) == {'plugin', 'input'}
                    and invocation['plugin']['sha256'] == manifest['arms'][arm]['sha256']
                    and invocation['input']['sha256'] == row['prepared_sha256'] == repeat['prepared_sha256']
                    == old['prepared_sha256'][cid + '/' + arm], 'completion would change a native invocation')
            artifacts = {name: {'path': str(blocks.checked(inputs['old_records']).parent / ref['path']),
                                'sha256': ref['sha256']} for name, ref in row['failure_artifacts'].items()}
            require(set(artifacts) == {'result', 'columns', 'log'}, 'retained first-call artifacts incomplete')
            requests.append({'cell': cid, 'arm': arm, 'prepared': invocation['input'],
                             'plugin': invocation['plugin'], 'first': artifacts})
            dependencies += [*invocation.values(), *artifacts.values()]
    require(set(old['qualified_wrappers']) == {c['id'] for c in context['cells']} - affected,
            'original wrapper coverage changed')
    bindings = {}
    verifier = resolve.verifier if hasattr(resolve, 'verifier') else VerificationPass()
    for ref in dependencies:
        verifier.check(ref)
        require(ref['path'] not in bindings or bindings[ref['path']] == ref['sha256'], 'conflicting evidence binding')
        bindings[ref['path']] = ref['sha256']
    return {'schema': 1, 'kind': KIND + '_plan', 'status': 'prospective', 'inputs': copy.deepcopy(inputs),
            'requests': requests, 'timeout_s': 120, 'budget_s': 1200, 'native_calls': 8,
            'stopping': 'exactly the eight missing repeats; stop on first failure; no retries or timing',
            'bindings': [{'path': p, 'sha256': bindings[p]} for p in sorted(bindings)]}


def load_plan(bound):
    plan = blocks.read(bound, sealed=True)
    require(plan == declare(plan['inputs']), 'science completion plan changed')
    return plan


def _metadata(plan, request):
    from routing_cases import inspect_case
    import routing_oracle
    context, manifest = blocks.load_context(plan['inputs']['new_context'])
    cell = next(c for c in context['cells'] if c['id'] == request['cell'])
    data = plan['inputs']['execution']['cells'][cell['id']]
    source = blocks.checked(data['source'])
    meta = inspect_case(source, cell['family'])
    require(meta['body_sha256'] == cell['body_sha256'] and meta['absorbed_terms'] == cell['absorbed_terms']
            and meta['categorical_intercepts'] == cell['categorical_intercepts'], 'actual B model changed')
    witness_path = blocks.checked(data['witness'])
    witness = routing.read_json(witness_path)
    oracle = witness['independent_witness']
    oracle_path = (witness_path.parent / oracle['path']).resolve()
    require(digest(oracle_path) == oracle['sha256'], 'independent witness changed')
    meta['routing_oracle'], _ = routing_oracle.load_reference(oracle_path, cell['case_sha256'], cell['body_sha256'])
    return manifest, cell, meta, witness


def _read_call(artifacts, meta, reader):
    require(set(artifacts) == {'result', 'columns', 'log'}, 'call artifacts incomplete')
    paths = {name: blocks.checked(ref) for name, ref in artifacts.items()}
    raw = routing.read_json(paths['result'])
    require(raw.pop('columns') == [], 'completion requires binary output columns')
    validate_result(raw)
    detail = diagnostics(raw, False)
    require(paths['columns'].stat().st_size == 8 * meta['rows'] * meta['columns'], 'binary output size changed')
    expected = cmgschur.EXPECTED_SOLVER[reader]
    require(detail['solver_rows'][0][:2] == [expected, expected], 'explicit solver identity changed')
    match = re.search(r'NATIVE_SECONDS=([0-9.]+)', paths['log'].read_text())
    require(match is not None and math.isfinite(float(match[1])) and float(match[1]) > 0, 'missing native completion clock')
    signature = exact_view(raw, False)
    signature['columns_sha256'] = artifacts['columns']['sha256']
    detail['science_sha256'] = hashlib.sha256(json.dumps(signature, sort_keys=True).encode()).hexdigest()
    runner.check_oracle(raw, paths['columns'], meta)
    return raw, detail


def validate_first(plan, request):
    require(request in plan['requests'], 'undeclared completion request')
    manifest, cell, meta, witness = _metadata(plan, request)
    reader = runner.reader_arm(manifest, request['arm'])
    raw, detail = _read_call(request['first'], meta, reader)
    old = blocks.read(plan['inputs']['old_records'])
    baseline = next(r for r in old['records'] if r['cell'] == cell['id'] and r['arm'] == 'baseline')
    before = next(r for r in old['prerequisites'] if r['cell'] == cell['id'] and r['arm'] == 'baseline')
    require(before['status'] == baseline['status'] == 'passed'
            and before['science_sha256'] == baseline['science_sha256'], 'baseline diagnostic attestation changed')
    # Only diagnostic rows are reconstructed. This is never passed to a
    # scientific comparison: its earlier completion is attested by the old
    # pinned producer's unique two-field exception and execution order.
    diagnostic = {'return_code': 0, 'matrices': {'RSOLVER': {'rows': 2, 'columns': 16,
        'bits': [struct.pack('>d', x).hex() for row in baseline['solver_rows'] for x in row]}}}
    scope = runner.cross_request_scope(diagnostic, raw, reader, meta)
    require(all('/matrices/RSOLVER/bits/' + str(i) in scope for i in [8, 9]), 'typed fallback correction does not apply')
    policy, preset = runner.resource_policy(manifest, request['arm'])
    resources = runner.resource_diagnostics(blocks.checked(request['first']['log']).read_text(), policy,
        meta['absorbed_terms'] == 0, witness, expected_preset=preset)
    return {'cell': cell['id'], 'arm': request['arm'], 'status': 'passed', 'certificate_pass': True,
            'science_pass': True, 'science_sha256': detail['science_sha256'], 'solver_rows': detail['solver_rows'],
            'prepared_sha256': request['prepared']['sha256'], 'witness_sha256': plan['inputs']['execution']['cells'][cell['id']]['witness']['sha256'],
            **resources, 'derivation': 'retained first call; reviewed original comparison exception'}


def validate_repeat(plan, request, call):
    require(call.get('cell') == request['cell'] and call.get('arm') == request['arm']
            and call.get('prepared') == request['prepared'], 'repeat request changed')
    process = call.get('process', {})
    require(process.get('dispatched') is True and process.get('process_completed') is True
            and process.get('call_limit_s') == 120,
            'repeat process was incomplete or used another limit')
    first = validate_first(plan, request)
    manifest, _, meta, _ = _metadata(plan, request)
    _, detail = _read_call(call['artifacts'], meta, runner.reader_arm(manifest, request['arm']))
    require(detail['science_sha256'] == first['science_sha256'], 'missing repeat changed exact science or solver signature')
    return {**first, 'repetition': 0, 'solver_rows': detail['solver_rows'],
            'derivation': 'one missing repeat; exact original first-call fingerprint including columns'}


def finalize(plan_bound, report_bound):
    plan = load_plan(plan_bound)
    report = blocks.read(report_bound)
    require(report.get('kind') == KIND + '_native_calls' and report.get('status') == 'complete'
            and report.get('plan') == plan_bound and len(report.get('calls', [])) == 8
            and type(report.get('elapsed_s')) in {int, float} and math.isfinite(report['elapsed_s'])
            and 0 <= report['elapsed_s'] <= plan['budget_s'], 'incomplete or over-budget missing-repeat cohort')
    claim_bound = report['claim']
    canonical = blocks.checked(plan['inputs']['old_records']).parent.parent / 'science-completion-native-claim.json'
    require(blocks.checked(claim_bound) == canonical, 'completion claim must be unique to the original cohort')
    claim = blocks.read(claim_bound)
    executor = claim.get('executor')
    review = blocks.read(plan['inputs']['transition_review'])
    require(isinstance(executor, dict) and executor == review.get('executor')
            and executor in review['evidence'],
            'native completion executor was not source-reviewed')
    blocks.checked(executor)
    require(claim.get('kind') == KIND + '_native_claim' and claim.get('plan') == plan_bound
            and type(claim.get('native_calls')) is int and claim['native_calls'] == 8
            and claim.get('requests') == [{k: r[k] for k in ['cell', 'arm', 'prepared']} for r in plan['requests']],
            'completion claim differs from the eight declared calls')
    first, repeats = [], []
    for request, call in zip(plan['requests'], report['calls']):
        first.append(validate_first(plan, request))
        repeats.append(validate_repeat(plan, request, call))
    return {'schema': 1, 'kind': KIND, 'status': 'complete', 'plan': plan_bound, 'native_report': report_bound,
            'first_checks': first, 'repeat_checks': repeats,
            'original_outcome': 'complete_with_failed_cells; preserved unchanged'}


def load_completion(context_bound, completion_bound):
    completion = blocks.read(completion_bound, sealed=True)
    require(completion.get('kind') == KIND and completion == finalize(completion['plan'], completion['native_report']),
            'science completion certificate does not reproduce actual evidence')
    plan = load_plan(completion['plan'])
    require(plan['inputs']['new_context'] == context_bound, 'completion used another current context')
    context, manifest, old, _, _ = _state(plan['inputs'])
    before = {(r['cell'], r['arm']): r for r in completion['first_checks']}
    after = {(r['cell'], r['arm']): r for r in completion['repeat_checks']}
    effective = {key: [copy.deepcopy((before if key == 'prerequisites' else after).get((r['cell'], r['arm']), r))
                       for r in old[key]] for key in ['prerequisites', 'records']}
    outcomes = {c['id']: runner.prerequisite_outcomes(manifest, blocks.required_arms(context, c),
        _rows(effective, 'prerequisites', c['id']), _rows(effective, 'records', c['id'])) for c in context['cells']}
    require(all(runner.proof_admissible(manifest, value) for value in outcomes.values()), 'derived cohort still has blocking science')
    wrappers = {cid: {'path': str(blocks.checked(plan['inputs']['old_records']).parent / ref['path']), 'sha256': ref['sha256']}
                for cid, ref in old['qualified_wrappers'].items()}
    for cid in {r['cell'] for r in plan['requests']}:
        wrappers[cid] = completion_bound
    return context, manifest, {'kind': KIND, 'completion': completion_bound, 'plan': plan,
        'prepared_sha256': copy.deepcopy(old['prepared_sha256']), 'qualified_wrappers': wrappers,
        'effective_rows': effective, 'effective_outcomes': outcomes}


def science_bundle(context, manifest, qualified, cell, witness_bound):
    plan, cid = qualified['plan'], cell['id']
    _, _, old, resolve, dependencies = _state(plan['inputs'])
    required = blocks.required_arms(context, cell)
    if cid in old['qualified_wrappers']:
        witness, proof, frozen = _proof(witness_bound, cell, manifest, required, old, resolve)
        dependencies += frozen
        witness_path = blocks.checked(witness_bound)
    else:
        witness_path = blocks.checked(plan['inputs']['execution']['cells'][cid]['witness'])
        witness = routing.read_json(witness_path)
        witness = {**witness, 'arm_outcomes': qualified['effective_outcomes'][cid]}
        proof = {'kind': KIND + '_derived_cell', 'prerequisites': _rows(qualified['effective_rows'], 'prerequisites', cid),
                 'repeat_checks': _rows(qualified['effective_rows'], 'records', cid)}
    certificate = blocks.read(qualified['completion'], sealed=True)
    report = blocks.read(certificate['native_report'])
    dependencies += [*plan['bindings'], qualified['completion'], certificate['plan'], certificate['native_report'], report['claim']]
    dependencies += [ref for call in report['calls'] for ref in call['artifacts'].values()]
    return {'witness': witness, 'proof': proof, 'witness_path': witness_path, 'dependencies': dependencies}
