#!/usr/bin/env python3
"""Paired native-time nonregression check with exact-science guards.

Runs each case through the REPLAY_TIMING harness (bench/build_timing_harness.sh)
for every plugin, one warm-up plus N repetitions, alternating plugin order each
repetition. Every run must reproduce the first run's exact science and solver
signature (scripts/replay_plugins.exact_view), so only arithmetic-preserving
candidates can be timed. The first --plugin is the baseline. For each other
plugin and case it reports the median native-time ratio and the upper bound of
a paired-median 95% bootstrap (10,000 resamples, fixed seed 739391), and passes
when that bound is within 1 + max(5%, 1 ms / baseline median).

Outputs go to a new directory (normally under target/); plugin, case and
harness hashes are bound at start and re-checked at the end.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import random
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from replay_plugins import exact_view, matrix  # noqa: E402

BOOTSTRAP_SEED = 739391
BOOTSTRAP_DRAWS = 10_000
RELATIVE_ALLOWANCE = 0.05
ABSOLUTE_ALLOWANCE_S = 0.001


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def compare(base: list[float], candidate: list[float]) -> dict:
    ratios = [c / b for c, b in zip(candidate, base)]
    rng = random.Random(BOOTSTRAP_SEED)
    draws = sorted(statistics.median(rng.choices(ratios, k=len(ratios)))
                   for _ in range(BOOTSTRAP_DRAWS))
    allowance = 1 + max(RELATIVE_ALLOWANCE, ABSOLUTE_ALLOWANCE_S / statistics.median(base))
    lower, upper = draws[int(0.025 * BOOTSTRAP_DRAWS) - 1], draws[int(0.975 * BOOTSTRAP_DRAWS) - 1]
    return {'median_ratio': statistics.median(ratios), 'bootstrap95': [lower, upper],
            'upper_allowance': allowance, 'nonregression_pass': upper <= allowance}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--plugin', action='append', required=True,
                        help='label=path; the first is the baseline')
    parser.add_argument('--cases', type=Path, nargs='+', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repetitions', type=int, default=7)
    parser.add_argument('--budget', type=float, default=1800, help='total seconds')
    parser.add_argument('--timeout', type=float, default=90, help='seconds per process')
    parser.add_argument('--harness', type=Path, default=ROOT / 'target/timing-harness/timing-harness')
    parser.add_argument('--result-changing', action='store_true',
                        help='declared result-changing candidate: require exact repeats per plugin; '
                             'cross-plugin agreement comes from the scoped replay')
    args = parser.parse_args()

    plugins = {label: Path(path).resolve()
               for label, path in (item.split('=', 1) for item in args.plugin)}
    if len(plugins) < 2 or not 5 <= args.repetitions <= 15:
        parser.error('need two or more plugins and 5-15 repetitions')
    cases = [case.resolve() for case in args.cases]
    if len(cases) != len(set(cases)):
        parser.error('cases must be distinct')
    harness = args.harness.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)

    start = time.monotonic()
    records: list[dict] = []
    expected: dict[str, str] = {}
    meta = {
        'status': 'running', 'platform': platform.platform(),
        'plugins': {name: {'path': str(path), 'sha256': sha256_file(path)} for name, path in plugins.items()},
        'cases': {str(case): sha256_file(case) for case in cases},
        'harness_sha256': sha256_file(harness), 'repetitions': args.repetitions, 'warmups': 1,
        'budget_s': args.budget, 'timeout_s': args.timeout,
        'clock': 'CLOCK_MONOTONIC around the native call', 'allowance': 'max(5%, 1 ms)',
        'records': records, 'summary': [],
    }

    def save() -> None:
        (output / 'summary.json').write_text(json.dumps(meta, indent=2) + '\n')

    def remaining() -> float:
        left = args.budget - (time.monotonic() - start)
        if left <= 0:
            raise TimeoutError('total timing budget exhausted')
        return min(args.timeout, left)

    try:
        save()
        for case in cases:
            poisson = case.name.startswith('ppml')
            for repetition in range(-1, args.repetitions):
                order = list(plugins)
                if repetition % 2:
                    order.reverse()
                for name in order:
                    result = output / 'current.json'
                    began = time.monotonic()
                    run = subprocess.run([str(harness), str(plugins[name]), '--replay', str(case), str(result)],
                                         capture_output=True, text=True, timeout=remaining())
                    if run.returncode:
                        raise RuntimeError(f'{case.name}/{name}: {run.stderr[-1000:]}')
                    data = json.loads(result.read_text())
                    if data['return_code'] != 0 or matrix(data, 'RREC')[-1] != 1 or matrix(data, 'RCERT')[-1] != 1:
                        raise RuntimeError(f'{case.name}/{name}: {data.get("error")}')
                    columns = result.with_suffix('.json.columns')
                    exact = exact_view(data, poisson)
                    exact['columns_sha256'] = sha256_file(columns)
                    fingerprint = hashlib.sha256(
                        json.dumps(exact, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
                    key = (case.name, name) if args.result_changing else case.name
                    if key not in expected:
                        expected[key] = fingerprint
                        suffix = f'-{name}' if args.result_changing else ''
                        (output / f'{case.stem}{suffix}-science.json').write_text(json.dumps(exact) + '\n')
                    if fingerprint != expected[key]:
                        raise RuntimeError(f'{case.name}/{name}/r{repetition}: nonexact science or solver signature')
                    seconds = float(re.search(r'NATIVE_SECONDS=([0-9.]+)', run.stderr)[1])
                    records.append({'case': case.name, 'plugin': name, 'repetition': repetition,
                                    'seconds': seconds, 'process_seconds': time.monotonic() - began,
                                    'science_sha256': fingerprint})
                    result.unlink()
                    columns.unlink()
                    save()
            base = next(iter(plugins))
            times = {name: [r['seconds'] for r in records
                            if r['case'] == case.name and r['plugin'] == name and r['repetition'] >= 0]
                     for name in plugins}
            row = {'case': case.name, 'medians': {n: statistics.median(v) for n, v in times.items()},
                   'comparisons': {name: compare(times[base], times[name]) for name in list(plugins)[1:]}}
            meta['summary'].append(row)
            save()
            print(json.dumps(row), flush=True)
        if any(sha256_file(path) != meta['plugins'][name]['sha256'] for name, path in plugins.items()):
            raise RuntimeError('plugin changed during timing')
        if any(sha256_file(case) != meta['cases'][str(case)] for case in cases):
            raise RuntimeError('case changed during timing')
        if sha256_file(harness) != meta['harness_sha256']:
            raise RuntimeError('harness changed during timing')
        meta['all_exact'] = True
        meta['exactness'] = 'per-plugin repeats' if args.result_changing else 'all plugins'
        meta['nonregression_pass'] = all(c['nonregression_pass'] for row in meta['summary']
                                         for c in row['comparisons'].values())
        meta['status'] = 'complete'
        meta['elapsed_s'] = time.monotonic() - start
        save()
    except BaseException as error:
        meta['status'] = 'failed'
        meta['error'] = str(error)
        save()
        raise
    return 0 if meta['nonregression_pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
