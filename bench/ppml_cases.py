#!/usr/bin/env python3
"""Campaign-DGP PPML replay cases for the exact-timing protocol.

Writes `ppml-<design>-n<rows>-t<threads>.case` files from the maintained
campaign generator (`campaign.make_dataset`, policy seed) with the PPML two-FE
protocol fixed since the beta.7 lean record: 16 regressors, robust VCE,
`separation(fe simplex ir)`, tolerance 1e-8, iterate 10000. The canonical Rust
serializer (`replay_large --ppml`) supplies the request metadata; only the
numeric columns are replaced. Cases are inputs for `time_exact.py`, which
guards exact science between plugins; they are not scientific evidence.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'bench'))
from native_io import case_header, digest, prepare_case  # noqa: E402

DESIGNS = ['two-sparse', 'akm-baseline', 'two-dense']
OUTPUTS = ['sample', 'residual', 'drop_reason', 'xb', 'absorbed', 'eta', 'mean']


def template_command(generator: Path, path: Path, regressors: int) -> list[str]:
    """The canonical PPML request: separation, tolerance and iterate fixed by the protocol."""
    return [str(generator), '--ppml', '--output', str(path), '--rows', '80', '--levels', '20,10',
            '--regressors', str(regressors), '--weights', 'unit', '--vce', 'robust',
            '--graph', 'random-mobility', '--solver', 'auto', '--seed', '20261013',
            '--tolerance', '1e-8', '--iterations', '10000', '--separation', 'fe simplex ir']


def frame_columns(frame, regressors: int) -> list[np.ndarray]:
    """Outcome, regressors, then the two FE identifiers, as the case layout expects."""
    names = ['y', *[f'x{j}' for j in range(1, regressors + 1)], 'g1', 'g2']
    return [np.ascontiguousarray(frame[name].to_numpy(dtype=np.float64)) for name in names]


def write_ppml_case(generator: Path, path: Path, columns: list[np.ndarray]) -> None:
    template = path.with_suffix('.template.case')
    subprocess.run(template_command(generator, template, len(columns) - 3),
                   check=True, stdout=subprocess.DEVNULL, timeout=120)
    try:
        with template.open('rb') as stream:
            shape, arguments = case_header(stream)
            for _ in range(shape[1]):
                if stream.readline() != b'0\n':
                    raise ValueError('numeric template required')
                for _ in range(shape[0]):
                    stream.readline()
            metadata = stream.read()
        if arguments[0] != b'estimate-ppml':
            raise ValueError('PPML template expected')
        fields = dict(a.decode().split('=', 1) for a in arguments[1:])
        n = len(columns[0])
        if any(len(c) != n for c in columns) or not all(np.all(np.isfinite(c)) for c in columns):
            raise ValueError('columns must be finite and of equal length')
        data = [np.ones(n), *columns]
        if len(data) != int(fields['output_sample']) - 1:
            raise ValueError('canonical template input layout changed')
        for name in OUTPUTS:
            initial = 1.0 if name == 'sample' else 0.0 if name == 'drop_reason' else float.fromhex('0x1p+1023')
            if int(fields['output_' + name]) != len(data) + 1:
                raise ValueError('canonical template output layout changed: ' + name)
            data.append(np.full(n, initial))
        if len(data) != shape[1]:
            raise ValueError('canonical template width changed')
        with path.open('xb') as stream:
            stream.write(('FEREG_REPLAY_V1\n' + ' '.join(map(str, [n, *shape[1:]])) + '\n').encode())
            for value in arguments:
                stream.write(str(len(value)).encode() + b'\n' + value + b'\n')
            for column in data:
                stream.write(b'0\n')
                for first in range(0, n, 32768):
                    bits = np.asarray(column[first:first + 32768], dtype=np.float64).view(np.uint64)
                    stream.write(''.join(f'{int(v):016x}\n' for v in bits).encode())
            stream.write(metadata)
    finally:
        template.unlink(missing_ok=True)


def prepare(output: Path, generator: Path, threads: list[int], rows: list[int], designs: list[str]) -> dict:
    import pandas as pd
    import campaign
    cases, data = output / 'cases', output / 'data'
    cases.mkdir(parents=True, exist_ok=True)
    data.mkdir(parents=True, exist_ok=True)
    regressors = campaign.POLICY['controls']
    manifest = []
    for design in designs:
        for n in rows:
            cell = dict(model='ppml', design=design, rows=n, threads=threads[0], vce='robust', save_fe=False,
                        section='exact-timing', regressors=regressors, weights='unit', effects=2,
                        id=f'ppml-{design}-n{n}')
            dta = data / (cell['id'] + '.dta')
            if not dta.exists():
                campaign.make_dataset(cell, dta)
            frame = pd.read_stata(dta, convert_categoricals=False)
            base = cases / (cell['id'] + '.case')
            if not base.exists():
                write_ppml_case(generator, base, frame_columns(frame, regressors))
            entry = dict(id=cell['id'], design=design, rows=n, base_sha256=digest(base), threads={})
            for count in threads:
                target = cases / f"{cell['id']}-t{count}.case"
                if not target.exists():
                    prepare_case(base, target, count)
                entry['threads'][str(count)] = digest(target)
            manifest.append(entry)
    result = dict(schema=1, seed=campaign.POLICY['seed'], regressors=regressors,
                  protocol='robust VCE, separation fe simplex ir, tolerance 1e-8, iterate 10000',
                  generator_sha256=digest(generator), cases=manifest)
    (output / 'cases.json').write_text(json.dumps(result, indent=1) + '\n')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', type=Path, required=True, help='task directory under target/tmp')
    parser.add_argument('--generator', type=Path, default=ROOT / 'target/ci/examples/replay_large')
    parser.add_argument('--threads', nargs='+', type=int, default=[4, 28])
    parser.add_argument('--rows', nargs='+', type=int, default=[1_000_000, 4_000_000])
    parser.add_argument('--designs', nargs='+', choices=DESIGNS, default=DESIGNS)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'target/tmp').resolve()):
        parser.error('output must be under target/tmp/<task>')
    if any(t < 1 or t > 1024 for t in args.threads) or any(r < 1000 for r in args.rows):
        parser.error('threads must be 1..1024 and rows at least 1000')
    result = prepare(output, args.generator.resolve(strict=True), args.threads, args.rows, args.designs)
    for entry in result['cases']:
        print(entry['id'], entry['base_sha256'][:16], sorted(entry['threads']))


if __name__ == '__main__':
    main()
