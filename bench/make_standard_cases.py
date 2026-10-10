#!/usr/bin/env python3
"""Regenerate the frozen timing workloads and verify their recorded hashes.

Builds the fereg-oracle `standard_cases` and `replay_large` examples, writes
the inputs to OUTPUT (default target/standard-cases) and fails unless every
file matches bench/standard_cases.json byte for byte.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_cases(output: Path, expected: dict[str, str]) -> list[str]:
    actual = {path.name for path in output.glob('*.case')}
    errors = [f'missing case: {name}' for name in sorted(expected.keys() - actual)]
    errors += [f'unlisted case: {name}' for name in sorted(actual - expected.keys())]
    errors += [f'hash mismatch: {name}' for name in sorted(actual & expected.keys())
               if sha256_file(output / name) != expected[name]]
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--output', type=Path, default=ROOT / 'target/standard-cases')
    args = parser.parse_args()
    spec = json.loads((ROOT / 'bench/standard_cases.json').read_text())
    subprocess.run(['cargo', 'build', '--locked', '-p', 'fereg-oracle', '--profile', 'ci',
                    '--example', 'standard_cases', '--example', 'replay_large'], cwd=ROOT, check=True)
    examples = ROOT / 'target/ci/examples'
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(examples / 'standard_cases'), str(output)], check=True)
    wide = spec['wide']
    for regressors in wide['regressors']:
        for threads in wide['threads']:
            case = output / f'ols-wide-k{regressors}-t{threads}.case'
            # replay_large requires a new file. Replace only declared outputs;
            # unlisted files remain visible to the verification below.
            if case.name in wide['cases']:
                case.unlink(missing_ok=True)
            subprocess.run([str(examples / 'replay_large'), '--output',
                            str(case),
                            '--rows', '50000', '--levels', '5000,500', '--regressors', str(regressors),
                            '--weights', 'pw', '--vce', 'robust', '--graph', 'random-mobility',
                            '--solver', 'auto', '--seed', '20261008', '--iterations', '1000',
                            '--threads', str(threads)], check=True, stdout=subprocess.DEVNULL)
    expected = {**spec['standard']['cases'], **wide['cases']}
    errors = verify_cases(output, expected)
    if errors:
        print('\n'.join(errors))
        return 1
    print(f'{len(expected)} frozen workloads verified in {output}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
