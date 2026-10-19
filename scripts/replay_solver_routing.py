#!/usr/bin/env python3
"""Bounded native IT, routing and PPML topology scientific qualification.

Preparation only changes framed options through maintained benchmark helpers.
The fixed 73-case replay remains separate. Forced routing is compared with the
same data/model under uninjected auto in the task-start plugin; old plugins do
not understand new developer injections. Overlap instead compares serial and
parallel preparation in the same candidate with fixed h8/k1 and analytic/QR
truth. No timings or performance claims.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'bench'))
import replay_plugins as replay

IT_CASES = ('ols-08', 'ols-09', 'ols-12', 'ols-13', 'ols-24', 'ols-25')
ROUTE_ARMS = ('h4-k0', 'h4-k1', 'h4-k2', 'diagonal', 'cells', 'direct')
PAIRS = tuple((source, 'it') for source in IT_CASES) + tuple(
    (source, arm) for source in ('ols-08', 'ols-25') for arm in ROUTE_ARMS
) + tuple((source, 'h4-k1') for source in ('ols-00', 'ols-04', 'ols-12', 'ols-13', 'ols-20'))
TOOLS = ('scripts/replay_solver_routing.py', 'scripts/replay_plugins.py',
         'scripts/replay_psd.py', 'scripts/replay_bias_reference.py',
         'crates/fereg-oracle/python/replay_ols_reference.py',
         'bench/native_io.py', 'bench/cmgschur.py', 'bench/routing.py')
DIRECT_CASES = ('direct-unit-fast', 'direct-unit-low', 'direct-weighted-fast', 'direct-weighted-low')
DIRECT_TOOLS = ('crates/fereg-oracle/examples/replay_direct.rs',
                'crates/fereg-oracle/examples/replay/wire.rs',
                'crates/fereg-oracle/tests/support/routing_direct.rs')
TOPOLOGY_CASES = tuple('topology-' + weight + '-' + mode
                       for weight in ('unit', 'weighted') for mode in ('fast', 'low'))
TOPOLOGY_FROZEN = tuple(f'ppml-{number:02}' for number in (0, 4, 9, 12, 20, 23, 24, 27, 30)) + (
    'ppml-separation-fe', 'ppml-separation-simplex', 'ppml-separation-ir', 'ppml-separation-mu',
    'ppml-bias-exact', 'ppml-bias-randomized')
TOPOLOGY_TOOLS = ('crates/fereg-oracle/examples/replay_topology.rs',
                  'crates/fereg-oracle/examples/replay/expected.rs',
                  'crates/fereg-oracle/examples/replay/wire.rs',
                  'tests/reference/ppml_bias_replay.json')
def body_digest(path):
    from native_io import case_header

    with path.open('rb') as stream:
        case_header(stream)
        return hashlib.sha256(stream.read()).hexdigest()


def prepare_injected_case(source, destination, stage):
    """Add one sanctioned injection to the maintained length-framed wire."""
    from native_io import case_header

    if stage not in {'it_coefficients', 'ppml_topology'}:
        raise ValueError('unsupported supplemental injection')
    with source.open('rb') as stream, destination.open('xb') as output:
        shape, arguments = case_header(stream)
        if any(arg.startswith((b'inject=', b'break_marker_token=')) for arg in arguments):
            raise ValueError('source already has a developer injection')
        if stage == 'it_coefficients' and (arguments[0] != b'estimate-ols'
                                           or b'solver=it' not in arguments):
            raise ValueError('coefficient IT witness requires explicit OLS IT')
        if stage == 'ppml_topology' and arguments[0] != b'estimate-ppml':
            raise ValueError('topology witness requires PPML')
        arguments.append(f'inject={stage}'.encode())
        shape[2] = len(arguments)
        output.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, shape)) + '\n').encode())
        for arg in arguments:
            output.write(str(len(arg)).encode() + b'\n' + arg + b'\n')
        shutil.copyfileobj(stream, output, 1 << 20)


def tool_hashes(direct=False, topology=False):
    names = (*TOOLS, *(DIRECT_TOOLS if direct else ()), *(TOPOLOGY_TOOLS if topology else ()))
    return {name: replay.digest(ROOT / name) for name in names}


def numerical_paths(result, expected):
    """Predeclare only existing finite numeric leaves and inner iteration counts."""
    view = replay.exact_view(result, False)
    fields = replay.numerical_scope_indices(False)
    for name, indices in replay.work_scope_indices(False).items():
        fields.setdefault(name, set()).update(indices)
    for name in ('RB', 'RV'):
        fields[name] = set(range(len(view['matrices'][name]['bits'])))
    paths = []
    for name, indices in fields.items():
        for index in sorted(indices):
            value = replay.number(view['matrices'][name]['bits'][index])
            if math.isfinite(value) and value < 8.98846567431158e307:
                paths.append(f'/matrices/{name}/bits/{index}')
    numeric_columns = {value for name, value in expected['outputs'].items()
                       if name not in {'sample', 'drop_reason'}} | set(expected.get('saved', []))
    paths += [f'/columns/{column}' for column in sorted(numeric_columns)]
    return paths


def prepare(baseline, output):
    import cmgschur
    import routing

    output = output.resolve()
    if not output.is_relative_to((ROOT / 'target/tmp/solver-routing').resolve()):
        raise ValueError('output must be under target/tmp/solver-routing')
    baseline = baseline.resolve()
    capture = json.loads((baseline / 'capture.json').read_text())
    if replay.digest(baseline / 'fereg_macos.plugin') != capture['identities']['plugin_sha256']:
        raise ValueError('baseline plugin differs from immutable capture')
    output.mkdir(parents=True, exist_ok=False)
    inputs = output / 'cases'
    inputs.mkdir()
    entries, files = [], {}
    for source, arm in PAIRS:
        original = baseline / 'replay/cases' / (source + '.case')
        expected_path = original.with_suffix('.json')
        for path in (original, expected_path):
            relative = str(path.relative_to(baseline))
            if replay.digest(path) != capture['replay_inputs'][relative]:
                raise ValueError('frozen case or independent target changed')
        expected = json.loads(expected_path.read_text())
        original_result = baseline / 'replay/results' / (source + '-plugin.json')
        paths = numerical_paths(json.loads(original_result.read_text()), expected)
        name = source + '-' + arm
        reference_name = source + ('-it-parent' if arm == 'it' else '-auto-parent')
        for workers in (1, 4):
            reference = inputs / f'{reference_name}-t{workers}.case'
            if not reference.exists():
                if arm == 'it':
                    cmgschur.prepare(original, reference, 'it', workers)
                else:
                    routing.prepare(original, reference, 'baseline', workers)
            candidate = inputs / f'{name}-t{workers}.case'
            if arm == 'it':
                prepare_injected_case(reference, candidate, 'it_coefficients')
            else:
                routing.prepare(original, candidate, arm, workers)
            if not body_digest(original) == body_digest(reference) == body_digest(candidate):
                raise ValueError('preparation changed model/data bytes')
            files[str(candidate.relative_to(output))] = replay.digest(candidate)
            files[str(reference.relative_to(output))] = replay.digest(reference)
        target = inputs / (source + '.json')
        if not target.exists():
            target.write_bytes(expected_path.read_bytes())
        files[str(target.relative_to(output))] = replay.digest(target)
        entries.append({'name': name, 'source': source, 'arm': arm,
                        'candidate_injection': 'it_coefficients' if arm == 'it' else None,
                        'reference': reference_name,
                        'expected': str(target.relative_to(output)),
                        'body_sha256': body_digest(original), 'allowed_paths': paths})
    manifest = {'schema': 1, 'baseline': str(baseline),
                'baseline_capture_sha256': replay.digest(baseline / 'capture.json'),
                'baseline_sha256': capture['identities']['plugin_sha256'],
                'harness_sha256': replay.digest(baseline / 'replay-harness'),
                'tools': tool_hashes(), 'files': files, 'cases': entries,
                'workers': [1, 4], 'repeats_per_worker': 2,
                'scope': 'Numerical coefficients/residuals/inference/FE diagnostics and inner '
                         'iterations only; sample/rank/omission/routes/discrete metadata exact.'}
    path = output / 'manifest.json'
    path.write_text(json.dumps(routing.seal(manifest), indent=2, sort_keys=True) + '\n')
    print(f'prepared {len(entries)} bounded cases: {path}')


def validate_direct_contract(expected, source):
    direct = expected['direct_reference']
    # Reported beta/covariance includes the normalized constant; the packed
    # absorption RHS additionally includes y. They are distinct dimensions.
    reported = direct['reported_columns']
    if (expected['rows'] != list(range(128)) or direct['finite_rank'] != 63
            or reported != direct['finite_rank'] + 1 or direct['outcome_columns'] != 1
            or direct['rhs'] != reported + direct['outcome_columns']
            or len(expected['beta']) != reported or len(expected['covariance']) != reported
            or any(len(row) != reported for row in expected['covariance'])
            or direct['require_admission'] != source.endswith('-fast')):
        raise ValueError('unexpected positive-direct witness shape or admission contract')


def prepare_direct(generated, baseline, output):
    import routing

    output, baseline = output.resolve(), baseline.resolve()
    if not output.is_relative_to((ROOT / 'target/tmp/solver-routing').resolve()):
        raise ValueError('output must be under target/tmp/solver-routing')
    capture = json.loads((baseline / 'capture.json').read_text())
    if replay.digest(baseline / 'fereg_macos.plugin') != capture['identities']['plugin_sha256']:
        raise ValueError('baseline plugin differs from immutable capture')
    output.mkdir(parents=True, exist_ok=False)
    inputs = output / 'cases'
    inputs.mkdir()
    entries, files = [], {}
    for source in DIRECT_CASES:
        original = inputs / (source + '.case')
        original.write_bytes((generated / original.name).read_bytes())
        target = original.with_suffix('.json')
        target.write_bytes((generated / target.name).read_bytes())
        expected = json.loads(target.read_text())
        direct = expected['direct_reference']
        validate_direct_contract(expected, source)
        for workers in (1, 4):
            for arm, name in [('baseline', source + '-parent'), ('direct', source)]:
                case = inputs / f'{name}-t{workers}.case'
                routing.prepare(original, case, arm, workers)
                if body_digest(case) != body_digest(original):
                    raise ValueError('direct preparation changed model/data bytes')
                files[str(case.relative_to(output))] = replay.digest(case)
        for path in (original, target):
            files[str(path.relative_to(output))] = replay.digest(path)
        entries.append({'name': source, 'source': source, 'arm': 'direct',
                        'source_case': str(original.relative_to(output)),
                        'reference': source + '-parent', 'expected': str(target.relative_to(output)),
                        'body_sha256': body_digest(original),
                        'allowed_paths': 'finite-baseline' if direct['require_admission'] else []})
    spec = {'schema': 1, 'positive_direct': True, 'baseline': str(baseline),
            'baseline_capture_sha256': replay.digest(baseline / 'capture.json'),
            'baseline_sha256': capture['identities']['plugin_sha256'],
            'harness_sha256': replay.digest(baseline / 'replay-harness'),
            'tools': tool_hashes(True), 'files': files, 'cases': entries,
            'workers': [1, 4], 'repeats_per_worker': 2,
            'scope': 'Fast: existing finite numerical leaves and inner iterations only. '
                     'Low: no allowances; the packed one-RHS direct attempt must decline.'}
    (output / 'manifest.json').write_text(json.dumps(routing.seal(spec), indent=2, sort_keys=True) + '\n')
    print(f'prepared four additive direct witnesses: {output / "manifest.json"}')


def direct_trace_errors(log, expected):
    events = []
    for line in log.splitlines():
        if line.startswith('FEREG_SOLVER_TRACE|phase=auto_direct_certified|'):
            try:
                fields = dict(part.split('=', 1) for part in line.split('|')[1:])
                events.append({name: int(fields[name]) for name in
                               ('rows', 'rhs', 'iterations', 'work', 'limit')})
            except (KeyError, ValueError):
                return ['malformed direct certification trace']
    contract = expected['direct_reference']
    if not contract['require_admission']:
        return ['Low fixture unexpectedly admitted direct work'] if events else []
    if not any(event['rows'] == 128 and event['rhs'] == contract['rhs']
               and event['iterations'] == 0 and 0 < event['work'] <= event['limit'] for event in events):
        return ['missing actual certified full-width direct admission']
    return []


def prepare_topology(generated, baseline, output):
    import routing
    from native_io import prepare_case

    output, baseline = output.resolve(), baseline.resolve()
    if not output.is_relative_to((ROOT / 'target/tmp/solver-routing').resolve()):
        raise ValueError('output must be under target/tmp/solver-routing')
    capture = json.loads((baseline / 'capture.json').read_text())
    if replay.digest(baseline / 'fereg_macos.plugin') != capture['identities']['plugin_sha256']:
        raise ValueError('baseline plugin differs from immutable capture')
    output.mkdir(parents=True, exist_ok=False)
    inputs = output / 'cases'
    inputs.mkdir()
    entries, files = [], {}
    for source in (*TOPOLOGY_CASES, *TOPOLOGY_FROZEN):
        additive = source in TOPOLOGY_CASES
        original = (generated if additive else baseline / 'replay/cases') / (source + '.case')
        target = original.with_suffix('.json')
        if not additive:
            for path in (original, target):
                if replay.digest(path) != capture['replay_inputs'][str(path.relative_to(baseline))]:
                    raise ValueError('frozen topology reference input changed')
        expected = json.loads(target.read_text())
        if expected['poisson'] is not True or (additive and len(expected['rows']) != 96):
            raise ValueError('unexpected topology witness shape')
        local = inputs / original.name
        local.write_bytes(original.read_bytes())
        local_target = inputs / target.name
        local_target.write_bytes(target.read_bytes())
        for workers in (1, 4):
            reference = inputs / f'{source}-parent-t{workers}.case'
            candidate = inputs / f'{source}-t{workers}.case'
            prepare_case(original, reference, workers)
            prepare_injected_case(reference, candidate, 'ppml_topology')
            if not body_digest(original) == body_digest(reference) == body_digest(candidate):
                raise ValueError('topology preparation changed model/data bytes')
            for path in (reference, candidate):
                files[str(path.relative_to(output))] = replay.digest(path)
        for path in (local, local_target):
            files[str(path.relative_to(output))] = replay.digest(path)
        entries.append({'name': source, 'source': source, 'arm': 'ppml_topology',
                        'source_case': str(local.relative_to(output)),
                        'reference': source + '-parent',
                        'expected': str(local_target.relative_to(output)),
                        'body_sha256': body_digest(original), 'allowed_paths': [],
                        'require_topology_reuse': additive})
    spec = {'schema': 1, 'ppml_topology': True, 'baseline': str(baseline),
            'baseline_capture_sha256': replay.digest(baseline / 'capture.json'),
            'baseline_sha256': capture['identities']['plugin_sha256'],
            'harness_sha256': replay.digest(baseline / 'replay-harness'),
            'tools': tool_hashes(topology=True), 'files': files, 'cases': entries,
            'workers': [1, 4], 'repeats_per_worker': 2,
            'scope': 'No scientific or iteration allowances; topology reuse must be exact.'}
    path = output / 'manifest.json'
    path.write_text(json.dumps(routing.seal(spec), indent=2, sort_keys=True) + '\n')
    print(f'prepared {len(entries)} exact PPML topology cases: {path}')


def topology_trace_errors(log, require_reuse):
    built, accepted, reused = {}, set(), False
    for line in log.splitlines():
        if not line.startswith('FEREG_SOLVER_TRACE|phase=ppml_topology|'):
            continue
        try:
            event = dict(part.split('=', 1) for part in line.split('|')[1:])
            values = {name: int(event[name]) for name in
                      ('sample', 'topology', 'weights', 'accepted', 'pending', 'bytes')}
            if (event['event'] not in {'built', 'reused', 'accepted', 'discarded'}
                    or any(value < 0 for value in values.values())
                    or values['accepted'] not in (0, 1) or values['pending'] not in (0, 1)):
                raise ValueError('invalid topology trace fields')
        except (KeyError, ValueError):
            return ['malformed PPML topology trace']
        geometry = values['sample'], values['topology']
        if event['event'] == 'built':
            built[geometry] = values['weights']
        elif event['event'] == 'accepted' and geometry in built and values['accepted'] == 1:
            accepted.add(geometry)
        elif (event['event'] == 'reused' and geometry in accepted and values['accepted'] == 1
              and values['weights'] > built[geometry]):
            reused = True
    return ['missing accepted topology reuse at changed weights'] if require_reuse and not reused else []


def direct_science_errors(result, expected, key='direct_reference'):
    if result['return_code']:
        return []  # The common scientific check reports failed transactions.
    errors = []
    contract = expected[key]
    # The public OLS wire stores each DoF row as (defined, value).
    dof = replay.matrix(result, 'RDOF')
    rows = len(expected['rows'])
    ranks = {0: rows, 1: rows, 2: rows, 3: rows, 5: contract['finite_rank'],
             7: contract['absorbed_rank'], 8: contract['finite_rank'] + contract['absorbed_rank'],
             9: contract['residual_dof'], 10: contract['residual_dof'], 19: 0}
    if any(len(dof) <= 2 * row + 1 or dof[2 * row] != 1 or dof[2 * row + 1] != target
           for row, target in ranks.items()):
        errors.append('independent direct sample/rank/DoF differs')
    status = result['matrices']['RSTATUS']
    wanted = [1.] * contract['finite_rank'] + [5.]
    if (status['columns'] != 4 or len(status['bits']) != 4 * len(wanted)
            or [replay.number(value) for value in status['bits'][1::4]] != wanted):
        errors.append('independent direct coefficient decisions differ')
    fit = replay.matrix(result, 'RFIT')
    if (len(fit) < 2 or fit[0] != 1 or not math.isfinite(fit[1])
            or abs(fit[1] - contract['weighted_rss']) > 1e-10 + 1e-7 * abs(contract['weighted_rss'])):
        errors.append('independent direct weighted RSS differs')
    targets = contract['saved_channels']
    if len(expected['saved']) != len(targets):
        errors.append('independent direct saved-FE layout differs')
    for index, target in zip(expected['saved'], targets):
        values = [replay.number(value) for value in result['columns'][index]]
        if len(values) != len(target) or any(
                not math.isfinite(value) or abs(value - wanted) > 1e-9 + 1e-7 * abs(wanted)
                for value, wanted in zip(values, target)):
            errors.append('independent component-centered saved-FE gauge differs')
    return errors


def traced_case(harness, plugin, case, output):
    trace = output.with_suffix('.trace')
    if trace.exists():
        raise ValueError('refuse stale direct admission trace')
    keys = ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE')
    prior = {key: os.environ.get(key) for key in keys}
    try:
        os.environ.update(FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(trace))
        result = replay.run_case(harness, plugin, case, output)
    finally:
        for key, value in prior.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    return result, trace.read_text() if trace.exists() else ''


def run(manifest_path, plugin, plugin_sha256):
    import routing

    spec = routing.unseal(json.loads(manifest_path.read_text()))
    directory = manifest_path.resolve().parent
    baseline = Path(spec['baseline'])
    plugin = plugin.resolve()
    if replay.digest(plugin) != plugin_sha256:
        raise ValueError('candidate plugin SHA-256 does not match')
    if spec.get('overlap', False):
        raise ValueError('the overlap replay panel was retired with the overlap experiment')
    if tool_hashes(spec.get('positive_direct', False), spec.get('ppml_topology', False)) != spec['tools']:
        raise ValueError('replay tools changed after preparation')
    if replay.digest(baseline / 'capture.json') != spec['baseline_capture_sha256']:
        raise ValueError('baseline capture changed after preparation')
    capture = json.loads((baseline / 'capture.json').read_text())
    for source in {entry['source'] for entry in spec['cases'] if 'source_case' not in entry}:
        relative = 'replay/cases/' + source + '.case'
        if replay.digest(baseline / relative) != capture['replay_inputs'][relative]:
            raise ValueError('frozen independent reference input changed')
    for relative, digest in spec['files'].items():
        if replay.digest(directory / relative) != digest:
            raise ValueError('prepared replay input changed')
    if replay.digest(baseline / 'fereg_macos.plugin') != spec['baseline_sha256']:
        raise ValueError('baseline plugin changed')
    harness = baseline / 'replay-harness'
    if replay.digest(harness) != spec['harness_sha256']:
        raise ValueError('frozen replay harness changed')
    output = directory / 'results'
    output.mkdir(exist_ok=False)
    references, records = {}, []
    for entry in spec['cases']:
        original = (directory / entry['source_case'] if 'source_case' in entry else
                    baseline / 'replay/cases' / (entry['source'] + '.case'))
        expected = json.loads((directory / entry['expected']).read_text())
        poisson = expected['poisson']
        expected = (replay.enrich_expected if poisson else replay.enrich_ols_expected)(original, expected)
        by_worker, first_decisions = {}, None
        for workers in spec['workers']:
            reference_case = directory / 'cases' / f'{entry["reference"]}-t{workers}.case'
            if reference_case.stem not in references:
                reference_plugin = plugin if entry.get('reference_artifact') == 'candidate' else baseline / 'fereg_macos.plugin'
                references[reference_case.stem] = replay.run_case(harness, reference_plugin,
                    reference_case, output / (reference_case.stem + '.json'))
            reference = references[reference_case.stem]
            science = replay.scientific_errors(reference, expected)
            direct = 'direct_reference' in expected
            independent = 'direct_reference' if direct else None
            if independent:
                science += direct_science_errors(reference, expected, independent)
            case = directory / 'cases' / f'{entry["name"]}-t{workers}.case'
            allowed = (numerical_paths(reference, expected) if entry['allowed_paths'] == 'finite-baseline'
                       else entry['allowed_paths'])
            if workers == 1 and entry.get('one_worker_exact', False):
                allowed = []
            scope = {'cases': {case.stem: {'input_sha256': replay.digest(case),
                'reason': spec['scope'], 'allowed_paths': allowed}}}
            prefixes = replay.affected_prefixes(scope, case, expected)
            admission, reference_repeat, trace_repeat = [], [], []
            if direct or spec.get('ppml_topology', False):
                candidate, trace = traced_case(harness, plugin, case, output / (case.stem + '.json'))
                repeated, repeated_trace = traced_case(harness, plugin, case,
                                                       output / (case.stem + '-repeat.json'))
                if direct:
                    admission = direct_trace_errors(trace, expected) + direct_trace_errors(repeated_trace, expected)
                else:
                    admission = topology_trace_errors(trace, entry['require_topology_reuse'])
                    admission += topology_trace_errors(repeated_trace, entry['require_topology_reuse'])
            else:
                candidate = replay.run_case(harness, plugin, case, output / (case.stem + '.json'))
                repeated = replay.run_case(harness, plugin, case, output / (case.stem + '-repeat.json'))
            replay.validate_scope_targets(prefixes, reference, poisson)
            replay.validate_scope_targets(prefixes, candidate, poisson)
            numeric = replay.differences(replay.exact_view(reference, poisson),
                                         replay.exact_view(candidate, poisson))
            exact = [path for path in numeric if not replay.allowed_difference(path, prefixes)]
            repeat = replay.differences(replay.exact_view(candidate, poisson),
                                        replay.exact_view(repeated, poisson))
            discrete = replay.differences(replay.discrete_view(reference, expected),
                                          replay.discrete_view(candidate, expected))
            decisions = replay.discrete_view(candidate, expected)
            if first_decisions is None:
                first_decisions = decisions
            else:
                discrete += ['cross-worker' + path for path in
                             replay.differences(first_decisions, decisions)]
            science_errors = science + replay.scientific_errors(candidate, expected)
            science_errors += replay.scientific_errors(repeated, expected)
            if independent:
                science_errors += direct_science_errors(candidate, expected, independent)
                science_errors += direct_science_errors(repeated, expected, independent)
            by_worker[workers] = {'science_errors': science_errors, 'exact_differences': exact,
                                 'repeat_differences': repeat, 'discrete_differences': discrete,
                                 'admission_errors': admission,
                                 'reference_repeat_differences': reference_repeat,
                                 'trace_repeat_differences': trace_repeat,
                                 'intentional_differences': len(numeric) - len(exact)}
        passed = all(not row[key] for row in by_worker.values() for key in
                     ('science_errors', 'exact_differences', 'repeat_differences', 'discrete_differences',
                      'admission_errors', 'reference_repeat_differences', 'trace_repeat_differences'))
        records.append({'case': entry['name'], 'passed': passed, 'workers': by_worker})
        print(f'{entry["name"]}: passed={passed}', flush=True)
    summary = {'schema': 1, 'manifest_sha256': replay.digest(manifest_path),
               'candidate_sha256': plugin_sha256, 'baseline_sha256': spec['baseline_sha256'],
               'harness_sha256': spec['harness_sha256'], 'cases': records,
               'passed': all(row['passed'] for row in records)}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2, sort_keys=True) + '\n')
    return 0 if summary['passed'] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    preparation = commands.add_parser('prepare')
    preparation.add_argument('--baseline', type=Path, required=True)
    preparation.add_argument('--output', type=Path, required=True)
    direct = commands.add_parser('prepare-direct')
    direct.add_argument('--generated', type=Path, required=True)
    direct.add_argument('--baseline', type=Path, required=True)
    direct.add_argument('--output', type=Path, required=True)
    topology = commands.add_parser('prepare-topology')
    topology.add_argument('--generated', type=Path, required=True)
    topology.add_argument('--baseline', type=Path, required=True)
    topology.add_argument('--output', type=Path, required=True)
    execution = commands.add_parser('run')
    execution.add_argument('--manifest', type=Path, required=True)
    execution.add_argument('--plugin', type=Path, required=True)
    execution.add_argument('--plugin-sha256', required=True)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.baseline, args.output)
        return 0
    if args.command == 'prepare-direct':
        prepare_direct(args.generated, args.baseline, args.output)
        return 0
    if args.command == 'prepare-topology':
        prepare_topology(args.generated, args.baseline, args.output)
        return 0
    return run(args.manifest, args.plugin, args.plugin_sha256)


if __name__ == '__main__':
    raise SystemExit(main())
