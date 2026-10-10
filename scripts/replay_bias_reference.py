#!/usr/bin/env python3
"""Input-bound independent references for the two small PPML bias replay cases.

Normal replay uses only the standard library. To regenerate the compact reference:
  python scripts/replay_bias_reference.py --derive CASE_DIRECTORY
Derivation requires requirements-ppml-bias.txt and reads case inputs only, never
plugin outputs. Existing ppml_bias_oracle supplies dense Poisson solves, defining
contractions, independent Philox signs and grouped Monte Carlo covariance.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / 'tests/reference/ppml_bias_replay.json'
NAMES = ('ppml-bias-exact', 'ppml-bias-randomized')
SOURCES = ('scripts/replay_bias_reference.py', 'scripts/ppml_bias_oracle.py')


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_case(path: Path, expected_action: str = 'estimate-ppml'):
    """Read the bounded numeric replay format with strict length framing."""
    lines = iter(path.read_text().splitlines())
    if next(lines) != 'FEREG_REPLAY_V1':
        raise ValueError('unsupported replay format')
    rows, columns, argc, matrix_count, macros = map(int, next(lines).split())
    if not (0 < rows <= 1000 and 0 < columns <= 100 and
            0 < argc <= 200 and 0 < matrix_count <= 100 and macros == 0):
        raise ValueError('bias reference input exceeds its bounded numeric format')

    def framed():
        length, value = int(next(lines)), next(lines)
        if len(value.encode()) != length:
            raise ValueError('invalid replay length framing')
        return value

    arguments = [framed() for _ in range(argc)]
    if arguments[0] != expected_action:
        raise ValueError(f'replay reference requires {expected_action}')
    pairs = [argument.split('=', 1) for argument in arguments[1:]]
    args = dict(pairs)
    if len(args) != len(pairs):
        raise ValueError('duplicate replay argument')

    def numbers(count):
        return [struct.unpack('>d', bytes.fromhex(next(lines)))[0] for _ in range(count)]

    data = []
    for _ in range(columns):
        if next(lines) != '0':
            raise ValueError('bias reference requires numeric columns')
        data.append(numbers(rows))
    matrices = {}
    for _ in range(matrix_count):
        name = framed()
        nr, nc = map(int, next(lines).split())
        if name in matrices or not (0 < nr * nc <= 10000):
            raise ValueError('invalid replay matrix')
        values = numbers(nr * nc)
        matrices[name] = [values[i:i + nc] for i in range(0, len(values), nc)]
    if next(lines, None) is not None:
        raise ValueError('unexpected replay suffix')
    return args, data, matrices


def derive_bias(path: Path) -> dict:
    import numpy as np
    from ppml_bias_oracle import correction, fit_poisson, randomized_correction, support_summary

    args, data, matrices = read_case(path)
    # This is deliberately a bounded witness for the declared replay fixtures,
    # not a second implementation of general Stata model/sample construction.
    expected_bindings = [[1, 1, 0], [2, 2, 0]] + [[i + 2, 3, i] for i in range(1, 5)]
    expected_components = [
        [i, 1, i, 1, 1, 0, 0, 2, 1, i, i, 0, 0] for i in (1, 2)
    ]
    if (matrices['BIND'] != expected_bindings or matrices['C'] != expected_components or
            matrices['T'] != [[i, i, i, 1, 1, 0, 0, 0] for i in (1, 2)] or
            matrices['A'] != [[i, 1, 1, 0] for i in (1, 2)] or
            matrices['AC'] != [[1, 1, 1, 3], [2, 1, 1, 4]] or
            matrices['S'] != [[i, 0] for i in range(1, 5)] or
            args['adjustment'] != 'none' or args['adjustment_position'] != '0' or
            args['include_intercept'] != '1' or args['separation'] != 'none' or
            args['cluster_term_count'] != '0' or len(data) != 14 or len(data[0]) != 120):
        raise ValueError('bias replay model differs from the declared two-FE numeric fixture')
    candidate, y, x1, x2, first, second = map(np.asarray, data[:6])
    unit = np.asarray(data[int(args['bias_unit']) - 1])
    if not np.all(candidate == 1) or not np.all(y > 0):
        raise ValueError('bias replay reference requires the complete positive-outcome sample')
    first_levels, second_levels = np.unique(first), np.unique(second)
    # Connected full dummy design: smallest second level is the canonical anchor.
    x = np.column_stack([first == level for level in first_levels] +
                        [second == level for level in second_levels[1:]] + [x1, x2])
    if np.linalg.matrix_rank(x) != x.shape[1]:
        raise ValueError('bias replay dummy design is not identified with one anchor')
    weights = np.ones(len(y))
    raw, mu, _ = fit_poisson(x, y, weights, np.ones(len(y)))
    method = args['bias_method']
    probes, seed, stream = (int(args['bias_' + key]) for key in ('probes', 'seed', 'stream'))
    if method == 'exact':
        if (probes, seed, stream) != (0, 0, 0):
            raise ValueError('exact replay has randomization controls')
        derivative, curvature = correction(x, y, weights, mu, unit)
        covariance = np.zeros((x.shape[1], 2, 2))
        groups = 1
    elif method == 'randomized' and probes == 32 and seed == 739391 and stream == 0:
        derivative, curvature, covariance = randomized_correction(
            x, y, weights, mu, unit, probes, seed, stream)
        groups = min(8, probes)
    else:
        raise ValueError('unsupported bias replay randomization controls')
    total = derivative + curvature
    variance_total = covariance[:, 0, 0] + covariance[:, 1, 1] + 2 * covariance[:, 0, 1]
    if np.any(variance_total < 0):
        raise ValueError('independent grouped total variance is negative')
    channels = (raw, derivative, curvature, total, raw + total,
                np.sqrt(covariance[:, 0, 0]), np.sqrt(covariance[:, 1, 1]),
                covariance[:, 0, 1], np.sqrt(variance_total))
    coefficients = [value for channel in channels for value in [*channel[-2:], 0.]]
    support = np.concatenate([support_summary(fe, unit, mu) for fe in (first, second)])
    if support[1] < 2 or support[12] < 2:
        raise ValueError('independent bias support screen failed')
    information = [float(mu[fe == level].sum())
                   for fe in (first, second) for level in np.unique(fe)]
    leverage = max(float(mu[fe == level].max() / mu[fe == level].sum())
                   for fe in (first, second) for level in np.unique(fe))
    units, counts = np.unique(unit, return_counts=True)
    return {
        'method': method, 'coefficient_columns': 3, 'zero_slots': [2],
        'coefficients': list(map(float, coefficients)), 'support': support.tolist(),
        'controls': {'0': 1 if method == 'exact' else 2, '1': len(units), '2': probes,
                     '3': seed, '4': stream, '5': groups,
                     '6': (len(units) if method == 'exact' else probes) + 2 * groups,
                     '14': int(counts.min()), '15': int(counts.max()),
                     '17': 1, '18': float(args['tolerance'])},
        # Ideal solve/reconstruction residuals are zero; the replay also checks
        # their sign and the unchanged requested certificate tolerance. Time,
        # workspace and route are implementation observations, not oracle targets.
        'diagnostics': {'7': 0., '8': 0., '11': float(np.max(np.abs(total))),
                        '12': float(mu.min()), '13': float(mu.max()),
                        '19': min(information), '20': leverage},
    }


def enrich_expected(case: Path, expected: dict, reference: Path = REFERENCE) -> dict:
    if case.stem not in NAMES:
        return expected
    pinned = json.loads(reference.read_text())
    if pinned['schema'] != 1 or pinned['sources'] != {name: digest(ROOT / name) for name in SOURCES}:
        raise RuntimeError('bias replay reference derivation source changed; regenerate independently')
    entry = pinned['cases'][case.stem]
    if digest(case) != entry['input_sha256']:
        raise RuntimeError('bias replay input differs from its independently derived reference')
    result = copy.deepcopy(expected)
    result['bias'] = entry['bias']
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--derive', type=Path, required=True, metavar='CASE_DIRECTORY')
    parser.add_argument('--output', type=Path, default=REFERENCE)
    args = parser.parse_args()
    result = {'schema': 1, 'sources': {name: digest(ROOT / name) for name in SOURCES},
              'cases': {name: {'input_sha256': digest(args.derive / (name + '.case')),
                               'bias': derive_bias(args.derive / (name + '.case'))}
                        for name in NAMES}}
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
