#!/usr/bin/env python3
"""Compare real plugin transactions exactly and against independent dense targets."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
from replay_psd import eigensystem

ROOT = Path(__file__).resolve().parents[1]


def number(bits: str) -> float:
    return struct.unpack('>d', bytes.fromhex(bits))[0]


def matrix(result: dict, name: str) -> list[float]:
    return [number(bits) for bits in result['matrices'][name]['bits']]


def exact_view(result: dict, poisson: bool) -> dict:
    view = copy.deepcopy(result)
    tables = view['matrices']
    del tables['RMEMORY']
    runtime = tables.pop('RRUNTIME')['bits']
    # Explicit field allowlist: keep route/convergence signatures, omit process telemetry.
    view['runtime_signature'] = runtime[1:11] if poisson else runtime[:6] + runtime[42:48] + runtime[54:60]
    solver = tables['RSOLVER']
    width = solver['columns']
    timing = {13, 14, 15} if poisson else {8, 9}
    solver['bits'] = [value for index, value in enumerate(solver['bits']) if index % width not in timing]
    if 'RBDIAG' in tables:
        tables['RBDIAG']['bits'][9] = 'elapsed-seconds'
    return view


def differences(a, b, path='') -> list[str]:
    if type(a) is not type(b):
        return [path + ': type']
    if isinstance(a, dict):
        if a.keys() != b.keys():
            return [path + ': keys']
        return [item for key in a for item in differences(a[key], b[key], f'{path}/{key}')]
    if isinstance(a, list):
        if len(a) != len(b):
            return [path + ': length']
        return [item for i, (x, y) in enumerate(zip(a, b)) for item in differences(x, y, f'{path}/{i}')]
    return [] if a == b else [path]


def expected_for_fixes(expected: dict, fixes: set[str]) -> dict:
    target = copy.deepcopy(expected)
    defect = target.get('known_defect')
    if defect in fixes:
        target['repaired_defect'] = target.pop('known_defect')
        target['return_code'] = 0
    return target


def allowed_difference(path: str, prefixes: list[str]) -> bool:
    return any(path == prefix or path.startswith(prefix + '/') for prefix in prefixes)


def scientific_errors(result: dict, expected: dict) -> list[str]:
    errors = []
    if result['return_code'] != expected['return_code']:
        return [f"return code {result['return_code']} != {expected['return_code']}"]
    if result['return_code']:
        if expected.get('error_contains', '') not in result.get('error', ''):
            errors.append('known-defect error identity differs')
        if matrix(result, 'RREC')[-1] != 0:
            errors.append('failed transaction marked complete')
        return errors
    if matrix(result, 'RREC')[-1] != 1:
        errors.append('receipt incomplete')
    def close(actual, target, label, rtol, atol):
        if len(actual) != len(target):
            errors.append(label + ': length')
            return
        for i, (a, b) in enumerate(zip(actual, target)):
            if not math.isfinite(a) or abs(a - b) > atol + rtol * max(abs(a), abs(b)):
                errors.append(f'{label}/{i}: {a:.12g} != {b:.12g}')
                break
    beta = expected['beta']
    beta_indices = expected.get('beta_indices', list(range(len(beta))))
    close([matrix(result, 'RB')[i] for i in beta_indices], beta, 'coefficients', 1e-7, 1e-10)
    width = result['matrices']['RV']['columns']
    covariance = matrix(result, 'RV')
    target = expected['covariance']
    close([covariance[i * width + j] for i in range(len(target)) for j in range(len(target))],
          [x for row in target for x in row], 'covariance', 1e-6, 1e-10)
    if expected.get('known_defect') == 'full-matrix-covariance':
        indices = expected['covariance_indices']
        actual = [[covariance[i * width + j] for j in indices] for i in indices]
        eigenvalues, _ = eigensystem(actual)
        _, intended = eigensystem(expected['independent_raw_covariance'])
        gap = max(abs(a-b) for left, right in zip(actual, intended) for a,b in zip(left,right))
        if min(eigenvalues) >= -2e-6 or gap <= 1e-6:
            errors.append('known full-matrix covariance defect was not reproduced')
    if expected.get('repaired_defect'):
        indices = expected['covariance_indices']
        actual = [[covariance[i * width + j] for j in indices] for i in indices]
        intended = expected['independent_raw_covariance']
        if expected['repaired_defect'] == 'full-matrix-covariance':
            _, intended = eigensystem(intended)
            eigenvalues, _ = eigensystem(actual)
            scale = max(1.0, max(abs(x) for row in actual for x in row))
            if min(eigenvalues) < -1e-10 * scale:
                errors.append('repaired full covariance is indefinite')
        close([x for row in actual for x in row], [x for row in intended for x in row],
              'independent repaired covariance', 1e-6, 1e-10)
    outputs = expected['outputs']
    sample = [number(x) for x in result['columns'][outputs['sample']]]
    if sample != expected['sample']:
        errors.append('sample mismatch')
    for name, target_name in [('residual', 'residual'), ('mean' if expected['poisson'] else 'xbd', 'fitted')]:
        values = result['columns'][outputs[name]]
        close([number(values[i]) for i in expected['rows']], expected[target_name], name, 1e-7, 1e-9)
    if expected.get('saved'):
        total = [sum(number(result['columns'][channel][i])
                     for channel in expected['saved']) for i in expected['rows']]
        absorbed = [number(result['columns'][outputs['absorbed']][i]) for i in expected['rows']]
        close(total, absorbed, 'identified saved FE sum', 1e-7, 1e-9)
    return errors


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_case(harness: Path, binary: Path, case: Path, output: Path) -> dict:
    environment = dict(os.environ)
    profile_pattern = None
    if environment.get('LLVM_PROFILE_FILE'):
        # Bind coverage to this native process, so Rust test/example profiles
        # cannot make an uninstrumented or uninitialized plugin appear covered.
        directory = Path(environment['LLVM_PROFILE_FILE']).parent
        profile_pattern = directory / (output.stem + '-*.profraw')
        if list(directory.glob(profile_pattern.name)):
            raise RuntimeError('stale native replay coverage profile')
        environment['LLVM_PROFILE_FILE'] = str(directory / (output.stem + '-%m.profraw'))
    completed = subprocess.run([str(harness), str(binary), '--replay', str(case), str(output)],
                               capture_output=True, text=True, timeout=120, env=environment)
    if completed.returncode != 0:
        raise RuntimeError(f'{case.stem}: harness/process failure {completed.returncode}: {completed.stderr[-2000:]}')
    if profile_pattern and not list(profile_pattern.parent.glob(profile_pattern.name)):
        raise RuntimeError(f'{case.stem}: native plugin emitted no coverage profile')
    return json.loads(output.read_text())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--next', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--cases', type=Path, default=ROOT / 'target/replay/cases')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/replay/results')
    parser.add_argument('--harness', type=Path, default=ROOT / 'target/actual-plugin-harness/actual_plugin_loader_test')
    parser.add_argument('--baseline-fixes', default='', help='comma-separated accepted fixes in the identified parent; empty means frozen legacy')
    parser.add_argument('--policy', type=Path, default=ROOT / 'tests/replay/intentional_changes.json')
    args = parser.parse_args()
    policy = json.loads(args.policy.read_text())
    candidate_fixes = set(policy['active'])
    baseline_fixes = set(filter(None, args.baseline_fixes.split(',')))
    if not baseline_fixes <= candidate_fixes or not candidate_fixes <= policy['repairs'].keys():
        raise RuntimeError('unknown or unapproved replay repair policy')
    transitions = candidate_fixes - baseline_fixes
    args.output.mkdir(parents=True, exist_ok=True)
    cases = sorted(args.cases.glob('*.case'))
    inventory = json.loads((ROOT / 'tests/replay/matrix.json').read_text())
    if [case.stem for case in cases] != inventory['cases']:
        raise RuntimeError('replay corpus differs from the declared bounded matrix')
    records = []
    for case in cases:
        expected = json.loads(case.with_suffix('.json').read_text())
        candidate = run_case(args.harness, args.next.resolve(), case, args.output / (case.stem + '-next.json'))
        baseline = run_case(args.harness, args.baseline.resolve(), case, args.output / (case.stem + '-baseline.json'))
        repeated = run_case(args.harness, args.next.resolve(), case, args.output / (case.stem + '-repeat.json'))
        repeat_changes = differences(exact_view(candidate, expected['poisson']), exact_view(repeated, expected['poisson']))
        changed = differences(exact_view(candidate, expected['poisson']), exact_view(baseline, expected['poisson']))
        if repeat_changes:
            changed.extend('fixed-thread-repeat' + path for path in repeat_changes)
        prefixes = [path for fix in transitions
                    if policy['repairs'][fix]['case'] == case.stem
                    for path in policy['repairs'][fix]['allowed_paths']]
        intentional = [path for path in changed if allowed_difference(path, prefixes)]
        changed = [path for path in changed if not allowed_difference(path, prefixes)]
        candidate_expected = expected_for_fixes(expected, candidate_fixes)
        science = scientific_errors(candidate, candidate_expected)
        baseline_science = scientific_errors(baseline, expected_for_fixes(expected, baseline_fixes))
        record = {'case': case.stem, 'input_sha256': digest(case), 'exact_differences': changed,
                  'science_errors': science, 'known_defect': candidate_expected.get('known_defect'), 'repaired_defect': candidate_expected.get('repaired_defect'), 'intentional_differences': intentional, 'baseline_science_errors': baseline_science, 'return_code': candidate['return_code']}
        records.append(record)
        print(f"{case.stem}: rc={candidate['return_code']} exact={len(changed)} science={science} repair={candidate_expected.get('repaired_defect')} intentional={len(intentional)} baseline_science={baseline_science}")
        if changed:
            print('  ' + ', '.join(changed[:5]))
    summary = {'schema': 1, 'next_sha256': digest(args.next), 'baseline_sha256': digest(args.baseline),
               'known_defect_cases': sum(bool(r['known_defect']) for r in records),
               'scientific_pass_cases': sum(not r['known_defect'] and not r['science_errors'] for r in records),
               'candidate_fixes': sorted(candidate_fixes), 'baseline_fixes': sorted(baseline_fixes), 'policy_sha256': digest(args.policy), 'cases': records, 'passed': all(not r['exact_differences'] and not r['science_errors'] and not r['baseline_science_errors'] for r in records)}
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    return 0 if summary['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
