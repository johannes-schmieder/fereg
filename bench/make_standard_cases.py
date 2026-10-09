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
            subprocess.run([str(examples / 'replay_large'), '--output',
                            str(output / f'ols-wide-k{regressors}-t{threads}.case'),
                            '--rows', '50000', '--levels', '5000,500', '--regressors', str(regressors),
                            '--weights', 'pw', '--vce', 'robust', '--graph', 'random-mobility',
                            '--solver', 'auto', '--seed', '20261008', '--iterations', '1000',
                            '--threads', str(threads)], check=True, stdout=subprocess.DEVNULL)
    expected = {**spec['standard']['cases'], **wide['cases']}
    mismatched = [name for name, digest in expected.items() if sha256_file(output / name) != digest]
    if mismatched:
        print('hash mismatch: ' + ', '.join(mismatched))
        return 1
    print(f'{len(expected)} frozen workloads verified in {output}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
