#!/usr/bin/env python3
"""Bounded, science-guarded native comparison of auto, CMG and CMG-Schur.

Build the plugin and replay_large first. Prepare cases with cmgschur_cases.py,
then run this script on a quiet host. One dense-oracle prerequisite per arm and
worker count precedes one warmup and seven rotating paired repetitions. Native
call time excludes replay input/output. Results are prototype observations, not
release qualification. Original-arm failures are retained as failed cells;
CMG-Schur failures stop dependent timings. Optional parent artifacts and explicit
arm subsets use the same guards. No automatic continuation/resampling.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import time

import numpy as np

from cmgschur_cases import scratch
from native import run_process
from native_io import ROOT, case_header, diagnostics, digest, numbers, validate_result
from replay_plugins import exact_view
from time_exact import compare

ARMS = ['auto', 'cmg', 'cmgschur4', 'cmgschur5', 'cmgschur6']
BASELINE_ARMS = ['baseline-' + arm for arm in ARMS if arm.startswith('cmgschur')]
EXPECTED_SOLVER = {'auto': 1, 'cmg': 7, 'cmgschur4': 8, 'cmgschur5': 8, 'cmgschur6': 8,
                   'lsmr-reference': 3}


def source_identity(root=ROOT):
    """Record live Git state or an explicitly unverified archive declaration."""
    identity = {'git': {'available': False, 'head': None, 'dirty': None},
                'tool_sha256': {name: digest(Path(__file__).with_name(name))
                               for name in ['cmgschur.py', 'cmgschur_cases.py']}}
    root = root.resolve()
    try:
        def git(*arguments):
            return subprocess.run(['git', '-C', str(root), *arguments], check=True,
                                  capture_output=True, text=True, timeout=5).stdout.strip()
        # An archive inside another checkout must not acquire its parent's HEAD.
        if Path(git('rev-parse', '--show-toplevel')).resolve() == root:
            head = git('rev-parse', 'HEAD')
            if re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', head):
                dirty = bool(git('status', '--porcelain', '--untracked-files=normal'))
                identity['git'] = {'available': True, 'head': head, 'dirty': dirty}
    except (OSError, subprocess.SubprocessError):
        pass  # Null means unavailable, never a clean worktree.
    declared = root / '.fereg-source.json'
    if not identity['git']['available'] and declared.is_file():
        with declared.open('rb') as stream:
            raw = stream.read(65537)
        if len(raw) > 65536:
            raise ValueError('source archive manifest exceeds 64 KiB')
        value = json.loads(raw)
        if not isinstance(value, dict):
            raise ValueError('source archive manifest must be a JSON object')
        identity['archive_manifest'] = {'sha256': hashlib.sha256(raw).hexdigest(),
                                        'declaration': value, 'verified_against_checkout': False}
    return identity


def solver_arm(arm):
    return arm.removeprefix('baseline-')


def artifact_for(arm):
    return 'baseline' if arm.startswith('baseline-') else 'candidate'


def selected_arms(requested, baseline):
    arms = list(requested if requested is not None else ARMS + (BASELINE_ARMS if baseline else []))
    if not arms or len(set(arms)) != len(arms) or any(a not in ARMS + BASELINE_ARMS for a in arms):
        raise ValueError('arms must be nonempty, unique supported labels')
    if not baseline and any(a in BASELINE_ARMS for a in arms):
        raise ValueError('baseline arms require --baseline-plugin')
    return arms


def order(repetition, selected=None):
    selected = ARMS if selected is None else selected
    if repetition == -1:
        return list(selected)
    # Mirror consecutive orders, then rotate; two arms must not cancel the
    # rotation against reversal and accidentally run in the same order forever.
    shift = (repetition // 2 + 1) % len(selected)
    arms = selected[shift:] + selected[:shift]
    return list(reversed(arms)) if repetition % 2 else arms


def prepare(source, destination, arm, threads):
    """Change length-framed options only; preserve the entire data/model body."""
    arm = solver_arm(arm)
    with source.open('rb') as stream, destination.open('xb') as out:
        shape, arguments = case_header(stream)
        if arguments[0] != b'estimate-ols':
            raise ValueError('prototype benchmark supports OLS only')
        updates = {'solver': 'cmgschur' if arm.startswith('cmgschur') else 'lsmr' if arm == 'lsmr-reference' else arm,
                   'threads': str(threads), 'preconditioner': 'auto' if arm in {'auto', 'lsmr-reference'} else 'off'}
        if arm.startswith('cmgschur'):
            updates['cmgdegree'] = arm[-1]
        if arm == 'lsmr-reference':
            updates['tolerance'] = '1e-12'
        arguments = [a for a in arguments if a.split(b'=', 1)[0].decode() not in {*updates, 'cmgdegree'}]
        arguments += [f'{key}={value}'.encode() for key, value in updates.items()]
        shape[2] = len(arguments)
        out.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, shape)) + '\n').encode())
        for arg in arguments:
            out.write(str(len(arg)).encode() + b'\n' + arg + b'\n')
        shutil.copyfileobj(stream, out, 1 << 20)
    return shape


def close(actual, expected, label, rtol, atol):
    a, b = np.asarray(actual), np.asarray(expected)
    if a.shape != b.shape or not np.all(np.isfinite(a)) or not np.all(np.isfinite(b)):
        raise ValueError(f'{label}: shape/nonfinite failure')
    error = np.abs(a - b)
    if np.any(error > atol + rtol * np.maximum(np.abs(a), np.abs(b))):
        raise ValueError(f'{label}: tolerance failure (maximum absolute difference {float(error.max()):.9g})')


def scientific_comparison(reference, candidate, reference_columns, columns, meta):
    """SPEC numerical envelopes and exact discrete/input/missingness decisions."""
    left, right = reference['matrices'], candidate['matrices']
    for name in ['RSTATUS', 'RFE', 'RCLUSTER', 'RSUBSET', 'RMARKB', 'RMARKV', 'RREC']:
        if left[name] != right[name]:
            raise ValueError(f'{name}: discrete scientific decision changed')
    for name in ['RFIT', 'RDOF', 'RCERT']:
        if left[name]['bits'][::2] != right[name]['bits'][::2]:
            raise ValueError(f'{name}: presence flags changed')
    ld, rd = numbers(left['RDOF']), numbers(right['RDOF'])
    for row in range(20):
        if row not in {4, 11} and ld[2*row+1] != rd[2*row+1]:
            raise ValueError(f'RDOF/{row}: discrete scientific decision changed')
    close(numbers(right['RB']), numbers(left['RB']), 'coefficients', 1e-7, 1e-10)
    close(numbers(right['RV']), numbers(left['RV']), 'covariance', 1e-6, 1e-10)
    close(numbers(right['RFIT']), numbers(left['RFIT']), 'fit', 1e-7, 1e-9)
    output = meta['outputs']
    numeric = {output[name] for name in ['residual', 'xb', 'xbd', 'absorbed']}
    a = np.memmap(reference_columns, dtype=np.float64, mode='r', shape=(meta['columns'], meta['rows']))
    b = np.memmap(columns, dtype=np.float64, mode='r', shape=a.shape)
    maximum = 0.0
    try:
        for column in range(meta['columns']):
            for start in range(0, meta['rows'], 65536):
                x, y = a[column, start:start+65536], b[column, start:start+65536]
                if column not in numeric:
                    if not np.array_equal(x.view(np.uint64), y.view(np.uint64)):
                        raise ValueError(f'column {column}: input/sample/drop decision changed')
                    continue
                retained = a[output['sample'], start:start+65536] == 1.0
                finite_x = np.isfinite(x) & (np.abs(x) < 8e307) & retained
                finite_y = np.isfinite(y) & (np.abs(y) < 8e307) & retained
                if not np.array_equal(finite_x, finite_y) or not np.array_equal(
                        x[~finite_x].view(np.uint64), y[~finite_x].view(np.uint64)):
                    raise ValueError(f'column {column}: missingness/excluded-row output changed')
                close(y[finite_x], x[finite_x], f'column {column}', 1e-7, 1e-9)
                if np.any(finite_x):
                    maximum = max(maximum, float(np.max(np.abs(y[finite_x] - x[finite_x]))))
    finally:
        del a, b
    return {'maximum_output_difference': maximum}


def oracle_check(result, columns, meta):
    target = meta['oracle']
    k = target['regressors']
    close(numbers(result['matrices']['RB'])[:k], target['beta'], 'dense coefficients', 1e-7, 1e-10)
    table = result['matrices']['RV']
    covariance = np.array(numbers(table)).reshape(table['rows'], table['columns'])[:k, :k]
    close(covariance, target['covariance'], 'dense covariance', 1e-6, 1e-10)
    data = np.memmap(columns, dtype=np.float64, mode='r', shape=(meta['columns'], meta['rows']))
    try:
        if not np.all(data[meta['outputs']['sample']] == 1):
            raise ValueError('dense fixture unexpectedly lost observations')
        for name, expected in [('residual', 'residual'), ('xbd', 'fitted')]:
            observed = data[meta['outputs'][name]]
            close(observed, target[expected], 'dense ' + name, 1e-7, 1e-9)
            close(observed, target[expected], 'CMG dense absolute ' + name, 0, 2e-8)
    finally:
        del data


def build_harness(path, rows, cells, timeout):
    system = platform.system()
    if system not in ['Darwin', 'Linux']:
        raise ValueError('timing harness requires macOS or Linux')
    command = [os.environ.get('CC', 'cc'), '-std=c11', '-O2', '-Wall', '-Wextra',
               '-Wpedantic', '-Wconversion', '-Wshadow', '-Wstrict-prototypes',
               '-Wmissing-prototypes', '-Werror', '-D_POSIX_C_SOURCE=200809L',
               '-DREPLAY_TIMING', '-DSYSTEM=' + ('APPLEMAC' if system == 'Darwin' else 'OPUNIX'),
               f'-DREPLAY_MAX_ROWS={rows}', f'-DREPLAY_MAX_CELLS={cells}ULL',
               '-I' + str(ROOT / 'plugin'), str(ROOT / 'tests/mock_spi/actual_plugin_loader_test.c'),
               '-lc' if system == 'Darwin' else '-ldl', '-o', str(path)]
    subprocess.run(command, check=True, capture_output=True, timeout=timeout)
    return command


def read_run(result_path, log, meta, arm):
    arm = solver_arm(arm)
    raw = json.loads(result_path.read_text())
    if raw.pop('columns') != []:
        raise ValueError('timing harness must return binary columns')
    validate_result(raw)
    record = diagnostics(raw, False)
    match = re.search(r'NATIVE_SECONDS=([0-9.]+)', log.read_text())
    if match is None or float(match[1]) <= 0:
        raise ValueError('missing/invalid monotonic native timing')
    record['seconds'] = float(match[1])
    columns = result_path.with_suffix('.json.columns')
    if columns.stat().st_size != 8 * meta['rows'] * meta['columns']:
        raise ValueError('binary output size mismatch')
    if record['solver_rows'][0][0] != EXPECTED_SOLVER[arm]:
        raise ValueError('requested solver identity mismatch')
    if arm != 'auto' and record['solver_rows'][0][1] != EXPECTED_SOLVER[arm]:
        raise ValueError('explicit solver was not used')
    signature = exact_view(raw, False)
    signature['columns_sha256'] = digest(columns)
    record['science_sha256'] = hashlib.sha256(json.dumps(signature, sort_keys=True).encode()).hexdigest()
    return raw, record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plugin', type=Path, required=True)
    parser.add_argument('--baseline-plugin', type=Path,
                        help='optional parent artifact; adds baseline-cmgschur4/5/6')
    parser.add_argument('--arms', nargs='+', choices=ARMS + BASELINE_ARMS,
                        help='explicit predeclared subset; default all applicable arms')
    parser.add_argument('--cases', type=Path, nargs='+', required=True)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--threads', type=int, nargs='+', default=[1, 4])
    parser.add_argument('--repetitions', type=int, default=7, choices=[7])
    parser.add_argument('--budget', type=float, default=1800)
    parser.add_argument('--timeout', type=float, default=90)
    args = parser.parse_args()
    try:
        arms = selected_arms(args.arms, args.baseline_plugin is not None)
    except ValueError as error:
        parser.error(str(error))
    if not (0 < args.timeout <= 3600 and 0 < args.budget <= 86400) or not args.threads or (
            len(set(args.threads)) != len(args.threads) or any(t < 1 or t > 1024 for t in args.threads)):
        parser.error('require unique positive workers, timeout<=3600 and budget<=86400')
    output = scratch(args.output)
    output.mkdir(parents=True, exist_ok=False)
    plugin = args.plugin.resolve(strict=True)
    plugins = {'candidate': plugin}
    if args.baseline_plugin:
        plugins['baseline'] = args.baseline_plugin.resolve(strict=True)
    cases = [args.oracle.resolve(strict=True), *(p.resolve(strict=True) for p in args.cases)]
    if len(set(cases)) != len(cases) or len({p.name for p in cases}) != len(cases):
        parser.error('oracle and timing cases must have distinct paths and basenames')
    started = time.monotonic()
    manifest = {'schema': 1, 'status': 'running', 'purpose': 'prototype observations, not release qualification',
                'host': platform.platform(), 'plugin_sha256': digest(plugin), 'threads': args.threads,
                'plugins': {label: {'sha256': digest(binary)} for label, binary in plugins.items()},
                'source_identity': source_identity(),
                'arms': arms, 'arm_artifacts': {arm: artifact_for(arm) for arm in arms},
                'warmups': 1, 'repetitions': args.repetitions,
                'budget_s': args.budget, 'timeout_s': args.timeout,
                'clock': 'CLOCK_MONOTONIC around one complete native call',
                'reference': 'untimed single-worker LSMR at tolerance 1e-12 per input and artifact; dense oracle on small fixtures',
                'stopping_rule': 'fixed panel, no automatic resampling; prototype failure stops dependent timing',
                'cases': {}, 'records': [], 'summary': []}
    def save():
        (output / 'summary.json').write_text(json.dumps(manifest, indent=2, allow_nan=False) + '\n')
    def remaining():
        left = args.budget - (time.monotonic() - started)
        if left <= 0:
            raise TimeoutError('total predeclared budget exhausted')
        return min(left, args.timeout)
    try:
        metadata = {}
        for path in cases:
            meta = json.loads(path.with_suffix('.json').read_text())
            if digest(path) != meta['case_sha256']:
                raise ValueError('case does not match its metadata hash')
            with path.open('rb') as stream:
                shape, _ = case_header(stream)
            if meta['rows'] != shape[0]:
                raise ValueError('case row count does not match metadata')
            meta['columns'] = shape[1]
            metadata[path] = meta
            manifest['cases'][path.name] = {k: v for k, v in meta.items() if k != 'oracle'}
        if 'oracle' not in metadata[cases[0]]:
            raise ValueError('--oracle must contain an independent dense reference')
        harness = output / 'timing-harness'
        manifest['harness_build_command'] = build_harness(harness, max(m['rows'] for m in metadata.values()),
            max(m['rows'] * m['columns'] for m in metadata.values()), remaining())
        manifest['harness_sha256'] = digest(harness)
        save()
        for index, path in enumerate(cases):
            meta = metadata[path]
            is_oracle = index == 0
            reference_path = output / 'reference.json'
            reference_columns = output / 'reference.columns'
            reference_input, reference_log = output / 'reference.case', output / 'reference.log'
            reference = None
            for artifact, binary in plugins.items():
                reference_arm = 'baseline-lsmr-reference' if artifact == 'baseline' else 'lsmr-reference'
                record = {'case': path.name, 'threads': 1, 'arm': reference_arm,
                          'plugin': artifact, 'plugin_sha256': manifest['plugins'][artifact]['sha256'],
                          'repetition': -2, 'phase': 'reference'}
                manifest['records'].append(record)
                columns = reference_path.with_suffix('.json.columns')
                try:
                    prepare(path, reference_input, reference_arm, 1)
                    record.update(run_process([str(harness), str(binary), '--replay',
                        str(reference_input), str(reference_path)], reference_log, remaining()))
                    raw, details = read_run(reference_path, reference_log, meta, reference_arm)
                    record.update(details)
                    if 'oracle' in meta:
                        oracle_check(raw, columns, meta)
                    if reference is None:
                        reference = raw
                        columns.rename(reference_columns)
                    else:
                        record.update(scientific_comparison(reference, raw, reference_columns, columns, meta))
                    record['status'] = 'passed'
                except Exception as error:
                    record['status'] = 'failed'
                    record['error'] = str(error)
                    record['log_tail'] = reference_log.read_text(errors='replace')[-2000:] if reference_log.exists() else ''
                    raise RuntimeError(f'{path.name}/{artifact}: strict LSMR reference prerequisite failed: {error}') from error
                finally:
                    reference_input.unlink(missing_ok=True)
                    reference_path.unlink(missing_ok=True)
                    reference_log.unlink(missing_ok=True)
                    columns.unlink(missing_ok=True)
                    save()
            for threads in args.threads:
                prepared = {}
                expected, failed = {}, set()
                for arm in arms:
                    target = output / f'current-{arm}.case'
                    prepare(path, target, arm, threads)
                    prepared[arm] = target
                repeats = [-1] if is_oracle else range(-1, args.repetitions)
                for repetition in repeats:
                    for arm in order(repetition, arms):
                        if arm in failed:
                            continue
                        remaining()
                        artifact = artifact_for(arm)
                        record = {'case': path.name, 'threads': threads, 'arm': arm,
                                  'plugin': artifact, 'plugin_sha256': manifest['plugins'][artifact]['sha256'],
                                  'repetition': repetition, 'phase': 'oracle' if is_oracle else 'timing'}
                        manifest['records'].append(record)
                        result_path = output / 'current.json'
                        columns = result_path.with_suffix('.json.columns')
                        log = output / 'current.log'
                        log.unlink(missing_ok=True)
                        try:
                            record.update(run_process([str(harness), str(plugins[artifact]), '--replay',
                                str(prepared[arm]), str(result_path)], log, remaining()))
                            raw, details = read_run(result_path, log, meta, arm)
                            record.update(details)
                            fingerprint = record['science_sha256']
                            if arm in expected and expected[arm] != fingerprint:
                                raise ValueError('nonexact fixed-worker repeat or changed solver signature')
                            expected.setdefault(arm, fingerprint)
                            record['science_sha256'] = fingerprint
                            if 'oracle' in meta:
                                oracle_check(raw, columns, meta)
                            record.update(scientific_comparison(reference, raw, reference_columns, columns, meta))
                            record['status'] = 'passed'
                        except (ValueError, RuntimeError, TimeoutError, subprocess.TimeoutExpired) as error:
                            record['status'] = 'failed'
                            record['error'] = str(error)
                            record['log_tail'] = log.read_text(errors='replace')[-2000:] if log.exists() else ''
                            failed.add(arm)
                            save()
                            if solver_arm(arm).startswith('cmgschur'):
                                raise RuntimeError(f'{path.name}/{arm}: prototype prerequisite failed: {error}') from error
                        finally:
                            result_path.unlink(missing_ok=True)
                            columns.unlink(missing_ok=True)
                            log.unlink(missing_ok=True)
                        save()
                for target in prepared.values():
                    target.unlink()
                if not is_oracle:
                    rows = [r for r in manifest['records'] if r['case'] == path.name and r['threads'] == threads
                            and r['repetition'] >= 0 and r['status'] == 'passed']
                    times = {a: [r['seconds'] for r in rows if r['arm'] == a] for a in arms}
                    complete = {a: values for a, values in times.items() if len(values) == args.repetitions and a not in failed}
                    row = {'case': path.name, 'threads': threads,
                           'medians': {a: statistics.median(v) for a, v in complete.items()},
                           'failed_arms': sorted(failed), 'comparisons_to_auto': {}, 'comparisons_to_cmg': {},
                           'candidate_over_baseline': {}}
                    for baseline in ['auto', 'cmg']:
                        if baseline in complete:
                            row['comparisons_to_' + baseline] = {a: compare(complete[baseline], values)
                                for a, values in complete.items() if a != baseline}
                    for baseline in BASELINE_ARMS:
                        candidate = solver_arm(baseline)
                        if baseline in complete and candidate in complete:
                            row['candidate_over_baseline'][candidate] = compare(complete[baseline], complete[candidate])
                    manifest['summary'].append(row)
                    print(json.dumps(row), flush=True)
                    save()
            reference_columns.unlink(missing_ok=True)
        if any(digest(binary) != manifest['plugins'][label]['sha256'] for label, binary in plugins.items()) or (
                digest(harness) != manifest['harness_sha256']):
            raise ValueError('plugin or harness changed during panel')
        if any(digest(path) != metadata[path]['case_sha256'] for path in cases):
            raise ValueError('input changed during panel')
        if source_identity() != manifest['source_identity']:
            raise ValueError('source identity or benchmark tooling changed during panel')
        manifest['status'] = 'complete_with_failed_cells' if any(
            r['status'] == 'failed' for r in manifest['records']) else 'complete'
        manifest['elapsed_s'] = time.monotonic() - started
        save()
        return 0 if manifest['status'] == 'complete' else 1
    except Exception as error:
        manifest['status'] = 'failed'
        manifest['error'] = f'{type(error).__name__}: {error}'
        manifest['elapsed_s'] = time.monotonic() - started
        save()
        print(manifest['error'], flush=True)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
