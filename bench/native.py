#!/usr/bin/env python3
"""Bounded, interleaved exact native A/B observations; no performance claims."""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import time

from native_io import (ROOT, cachegrind_counts, case_header, diagnostics, digest,
                       exact_differences, prepare_case, read_result)


def positive_int(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError('must be positive')
    return number


def seconds(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError('must be finite and positive')
    return number


def output_under_target(path):
    path = path.resolve()
    if not path.is_relative_to((ROOT / 'target').resolve()):
        raise ValueError('output must be under the repository target/ directory')
    return path


def build_harness(directory, rows, cells, timeout):
    system = platform.system()
    if system not in ('Darwin', 'Linux'):
        raise ValueError('native benchmark harness supports macOS/Linux only')
    binary = directory / 'actual_plugin_loader_test'
    command = [os.environ.get('CC', 'cc'), '-std=c11', '-O2', '-Wall', '-Wextra',
               '-Wpedantic', '-Wconversion', '-Wshadow', '-Wstrict-prototypes',
               '-Wmissing-prototypes', '-Werror',
               '-DSYSTEM=' + ('APPLEMAC' if system == 'Darwin' else 'OPUNIX'),
               f'-DREPLAY_MAX_ROWS={rows}', f'-DREPLAY_MAX_CELLS={cells}ULL',
               '-I' + str(ROOT / 'plugin'),
               str(ROOT / 'tests/mock_spi/actual_plugin_loader_test.c'),
               '-lc' if system == 'Darwin' else '-ldl', '-o', str(binary)]
    with (directory / 'build.log').open('wb') as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=timeout)
    return binary, command


def rss_bytes(value, system):
    if system == 'Darwin':
        return int(value)
    if system == 'Linux':
        return int(value) * 1024
    raise ValueError('unsupported RSS units')


def run_process(command, log, timeout):
    """Use wait4 for this child's RSS, not a cumulative RUSAGE_CHILDREN peak."""
    start = time.perf_counter()
    with log.open('xb') as output:
        process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        try:
            while True:
                child, status, usage = os.wait4(process.pid, os.WNOHANG)
                if child:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                if time.perf_counter() - start >= timeout:
                    raise TimeoutError(f'process exceeded {timeout}s; see {log}')
                time.sleep(0.01)
        except BaseException:
            os.killpg(process.pid, signal.SIGKILL)
            _, status, _ = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
            raise
    wall = time.perf_counter() - start
    if process.returncode:
        raise RuntimeError(f'process return code {process.returncode}; see {log}')
    return {'wall_s': wall, 'user_s': usage.ru_utime, 'system_s': usage.ru_stime,
            'maxrss_bytes': rss_bytes(usage.ru_maxrss, platform.system())}


def compare_run(references, label, candidate, poisson):
    """Every run must match both its own first run and the other plugin's first."""
    for reference_label, reference in references.items():
        changed = exact_differences(reference, candidate, poisson)
        if changed:
            raise ValueError(f'{label} differs from {reference_label}: {changed[:10]} ({len(changed)} paths)')
    references.setdefault(label, candidate)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plugin-a', type=Path, required=True)
    parser.add_argument('--plugin-b', type=Path, required=True)
    parser.add_argument('--cases', type=Path, nargs='+', required=True,
                        help='explicit predeclared case files, no implicit glob')
    parser.add_argument('--threads', type=positive_int, nargs='+', required=True)
    parser.add_argument('--repeats', type=positive_int, required=True, help='2..20 interleaved repetitions')
    parser.add_argument('--timeout', type=seconds, required=True, help='seconds per native process, <=3600')
    parser.add_argument('--budget', type=seconds, required=True, help='total wall-clock stopping budget in seconds, <=86400')
    parser.add_argument('--output', type=Path, required=True, help='new directory under target/')
    parser.add_argument('--max-rows', type=positive_int, default=4000000)
    parser.add_argument('--max-cells', type=positive_int, default=100000000)
    parser.add_argument('--cachegrind', action='store_true',
                        help='Linux only; threads=1 and each input has 50k..200k rows')
    parser.add_argument('--keep-outputs', action='store_true', help='retain large raw JSON after checks')
    args = parser.parse_args()
    if not 2 <= args.repeats <= 20 or args.timeout > 3600 or args.budget > 86400:
        parser.error('require repeats=2..20, timeout<=3600, budget<=86400')
    if max(args.threads) > 1024 or len(args.threads) != len(set(args.threads)):
        parser.error('threads must be unique and <=1024')
    if len(args.cases) > 32 or len(args.cases) != len(set(p.resolve() for p in args.cases)):
        parser.error('require 1..32 distinct case files')
    if args.max_rows > 2147483647 or args.max_cells > 2147483647:
        parser.error('harness row/cell limits must fit signed 32-bit integers')
    if args.cachegrind and (platform.system() != 'Linux' or args.threads != [1]):
        parser.error('cachegrind requires Linux and --threads 1')
    output = output_under_target(args.output)
    output.mkdir(parents=True, exist_ok=False)
    records = []
    manifest = {'schema': 1, 'status': 'running', 'host': platform.platform(),
                'mode': 'cachegrind' if args.cachegrind else 'native',
                'repeats': args.repeats, 'threads': args.threads,
                'timeout_s': args.timeout, 'budget_s': args.budget,
                'max_rows': args.max_rows, 'max_cells': args.max_cells,
                'records': records, 'summary': []}
    def save():
        (output / 'run.json').write_text(json.dumps(manifest, indent=2, allow_nan=False) + '\n')
    start = time.monotonic()
    def remaining():
        value = args.budget - (time.monotonic() - start)
        if value <= 0:
            raise TimeoutError('total native benchmark budget exhausted')
        return min(args.timeout, value)
    try:
        plugins = {'A': args.plugin_a.resolve(strict=True), 'B': args.plugin_b.resolve(strict=True)}
        hashes = {label: digest(path) for label, path in plugins.items()}
        manifest['plugins'] = {label: {'path': str(path), 'sha256': hashes[label]} for label, path in plugins.items()}
        prepared = []
        for index, source in enumerate(args.cases):
            remaining()
            source = source.resolve(strict=True)
            with source.open('rb') as stream:
                shape, _ = case_header(stream)
            if shape[0] > args.max_rows or shape[0] * shape[1] > args.max_cells:
                raise ValueError(f'{source}: exceeds declared harness bounds')
            if args.cachegrind and not 50000 <= shape[0] <= 200000:
                raise ValueError('cachegrind case must contain 50000..200000 rows')
            for threads in args.threads:
                remaining()
                path = output / f'case-{index}-t{threads}.case'
                meta = prepare_case(source, path, threads)
                prepared.append({'path': path, 'source': str(source), 'threads': threads,
                                 'sha256': digest(path), **meta})
        manifest['cases'] = [{**item, 'path': str(item['path'])} for item in prepared]
        harness, command = build_harness(output, args.max_rows, args.max_cells, remaining())
        harness_hash = digest(harness)
        manifest['harness'] = {'path': str(harness), 'sha256': harness_hash, 'build_command': command}
        save()
        for index, case in enumerate(prepared):
            references, times = {}, {'A': [], 'B': []}
            for repetition in range(args.repeats):
                for label in ('A', 'B') if repetition % 2 == 0 else ('B', 'A'):
                    remaining()
                    if digest(plugins[label]) != hashes[label] or digest(harness) != harness_hash or digest(case['path']) != case['sha256']:
                        raise ValueError('frozen plugin, harness or case identity changed')
                    prefix = output / f'case-{index}-{label}-r{repetition}'
                    result_path = prefix.with_suffix('.json')
                    command = [str(harness), str(plugins[label]), '--replay', str(case['path']), str(result_path)]
                    if args.cachegrind:
                        cg = prefix.with_suffix('.cachegrind')
                        command = ['valgrind', '--tool=cachegrind', '--cache-sim=no', '--branch-sim=no',
                                   '--error-exitcode=99', '--cachegrind-out-file=' + str(cg)] + command
                    record = {'case_index': index, 'plugin': label, 'repetition': repetition,
                              'threads': case['threads'], 'command': command}
                    records.append(record)
                    record.update(run_process(command, prefix.with_suffix('.log'), remaining()))
                    if digest(plugins[label]) != hashes[label] or digest(harness) != harness_hash or digest(case['path']) != case['sha256']:
                        raise ValueError('frozen identity changed during run')
                    result = read_result(result_path, case['rows'], case['columns'])
                    record.update(diagnostics(result['head'], case['poisson']))
                    compare_run(references, label, result, case['poisson'])
                    record['exact_differences'] = 0
                    record['columns_sha256'] = result['columns_sha256']
                    prefix.with_suffix('.head.json').write_text(json.dumps(result, allow_nan=False) + '\n')
                    if args.cachegrind:
                        record['instructions'] = cachegrind_counts(cg)
                    times[label].append(record[record['timing_field']])
                    if not args.keep_outputs:
                        result_path.unlink()
                    save()
            a, b = (statistics.median(times[label]) for label in ('A', 'B'))
            row = {'case_index': index, 'source': case['source'], 'threads': case['threads'],
                   'timing_field': 'total_s' if case['poisson'] else 'native_s',
                   'seconds': times, 'median_ratio_b_over_a': b / a if a > 0 else None,
                   'exact_differences': 0}
            manifest['summary'].append(row)
            print(json.dumps(row, allow_nan=False), flush=True)
        remaining()
        manifest['status'] = 'complete'
        save()
        return 0
    except Exception as error:
        manifest['status'] = 'failed'
        manifest['error'] = f'{type(error).__name__}: {error}'
        save()
        print(manifest['error'], file=__import__('sys').stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
