#!/usr/bin/env python3
"""Fit and freeze support from the complete, bound two-platform calibration.

This offline command never runs an estimation or edits the runtime/defaults.
Unsupported rows and keys are retained explicitly; it cannot request more data.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import asdict
from fractions import Fraction
from itertools import product
from pathlib import Path

import routing
import two_fe_policy as protocol
import two_fe_policy_fit as model
from two_fe_policy_labels import paired_cell

ROLES = ('packed', 'outcome', 'regressor', 'final')
REQUIRED_COVERAGE = frozenset(product(protocol.SEEDS, ('Darwin', 'Linux'), (1, 4, 8)))


def future_at(point):
    """Only pre-intervention history and the immutable remaining caller schedule."""
    role, width = point['role'], point['r']
    protocol.require(role in ROLES and width > 0 and point['stride'] >= width,
                     'invalid intervention role/shape')
    if role == 'packed':
        protocol.require(width == point['total'] + 1, 'packed width differs from demand')
        columns, block, final = 0, 1, True
    elif role == 'outcome':
        protocol.require(width == 1, 'outcome width differs from demand')
        columns, block, final = point['total'], point['block'], True
    elif role == 'regressor':
        protocol.require(0 <= point['start'] < point['end'] <= point['total']
                         and point['end'] - point['start'] == width
                         and width <= point['block'], 'regressor range differs from demand')
        columns, block, final = point['total'] - point['end'], point['block'], True
    else:
        protocol.require(width == 1, 'final width differs from demand')
        columns, block, final = 0, 1, False
    raw = [point['history_'+key] for key in ('width', 'actions', 'steps', 'live', 'original')]
    protocol.require(all(v is None for v in raw) or all(v is not None for v in raw),
                     'partial completed-action history')
    history_width = raw[0]
    history = model.history_work(*raw) if history_width is not None else None
    groups = model.grouped_future(columns, block, point['stride'], final,
                                  history_width=history_width, history=history)
    mode = 0
    # The final residual never uses regressor history, even when its width is 1.
    regressors = groups[:-1] if final else groups
    for group in regressors:
        if history_width is not None:
            mode = max(mode, 1 if group.width == history_width else 2)
    return groups, mode


def label_row(observation):
    point, labels = observation['checkpoint'], observation['labels']
    future, _ = future_at(point)
    return model.LabelRow(
        cell=tuple(observation[k] for k in ('seed', 'family', 'memory', 'workers', 'platform')),
        structure=model.Structure(*(point[k] for k in ('n', 'p', 'q', 'd'))),
        execution=model.Execution(point['r'], point['stride'], point['workers']),
        work=model.Work(point['forecast_h'], point['forecast_d']), future=future,
        incumbent_ns=labels['I'], setup_ns=labels['B'], startup_ns=labels['startup'],
        restart_ns=labels['R'], continuation_ns=labels['C'],
        observed_suffix_gain=labels['observed_suffix_gain'])


def keys(point):
    protocol.require(point['role'] in ROLES and point['weighted'] in (0, 1)
                     and point['scaled'] in (0, 1) and point['epoch'] in (0, 1)
                     and type(point['classes']) is int and 0 < point['classes'] < 8,
                     'invalid current support key')
    base = (((ROLES.index(point['role'])*2 + point['weighted'])*2
             + point['scaled'])*2 + point['epoch'])*3
    return tuple(base + progress for progress in range(3) if point['classes'] & (1 << progress))


def support(observations):
    """Full seed×platform×worker coverage, no family, size or timing fences."""
    coverage = defaultdict(set)
    for observation in observations:
        point = observation['checkpoint']
        witness = tuple(observation[k] for k in ('seed', 'platform', 'workers'))
        protocol.require(witness in REQUIRED_COVERAGE and point['workers'] == witness[2],
                         'support witness outside frozen calibration')
        groups, history_mode = future_at(point)
        for index in keys(point):
            coverage[index, 0].add(witness)
            if groups:
                coverage[index, history_mode+1].add(witness)
    table = [0]*96
    for (index, bit), witnesses in coverage.items():
        if witnesses == REQUIRED_COVERAGE:
            table[index] |= 1 << bit
    detail = [dict(index=index, bit=bit, witnesses=[list(x) for x in sorted(witnesses)],
                   supported=witnesses == REQUIRED_COVERAGE)
              for (index, bit), witnesses in sorted(coverage.items())]
    return table, detail


def admit(point, table):
    groups, mode = future_at(point)
    required = 1 | (1 << (mode+1) if groups else 0)
    return all(table[index] & required == required for index in keys(point))


def freeze(observations, unavailable):
    """One fit, one quantization, retained failure; no fallback coefficients."""
    result = dict(schema=1, protocol=protocol.MODE, status='unsupported',
                  observations=observations, unavailable=unavailable,
                  coefficients=None, rational=None, support=[0]*96, coverage=[], incidence=[])
    try:
        rows = [label_row(observation) for observation in observations]
        fitted = model.fit(rows)
        table, coverage = support(observations)
        incidence = []
        for observation, row in zip(observations, rows):
            eligible = admit(observation['checkpoint'], table)
            prices = model.evaluate(fitted.coefficients, row.structure, row.execution, row.work, row.future)
            incidence.append(dict(control=observation['control'], action=observation['action'],
                cell=list(row.cell), supported=eligible, build=eligible and prices.build,
                prices=asdict(prices)))
        result.update(status='fitted', coefficients=asdict(fitted.coefficients),
                      rational=fitted.rational, support=table, coverage=coverage, incidence=incidence,
                      rows=fitted.rows, cells=fitted.cells, suffix_rows=fitted.suffix_rows)
    except (model.Unsupported, model.Unknown) as error:
        result['reason'] = str(error)
    return result


def collect(manifest_path, directory):
    """Read only a completed report whose actual result files still match it."""
    import qualify as q
    from native_io import digest
    manifest, directory = q.read(manifest_path), Path(directory)
    protocol.validate(manifest)
    summary = q.read(directory/'summary.json')
    result_path = directory/'run-0-0/results.json'
    run = q.read(result_path)
    identity = routing.identity(manifest)
    protocol.require(summary['status'] == 'diagnostic_complete' and not summary['errors']
                     and summary['manifest_sha256'] == identity and summary['native_calls'] == 864
                     and run['status'] == 'complete' and not run['failures']
                     and run['manifest_sha256'] == identity and run['budget_compliant'],
                     'calibration is incomplete, failed or unbound')
    protocol.require(summary.get('calibration_result_sha256') == digest(result_path),
                     'report does not bind the actual calibration results')
    protocol.require(run['execution_sources'] == manifest['execution_sources'],
                     'calibration source binding changed')
    cases = {case['id']: case for case in manifest['cases']}
    expected = {f'{name}-t{workers}' for name, workers in product(cases, manifest['threads'])}
    protocol.require(len(run['cases']) == len(expected)
                     and {cell['id'] for cell in run['cases']} == expected,
                     'missing/duplicate calibration cells')
    observations, unavailable, bodies, inputs = [], [], {}, {}
    for cell in run['cases']:
        name, worker = cell['id'].rsplit('-t', 1)
        item = cases[name]
        memory = name.rsplit('-', 1)[1]
        metadata_path = q.resolve(manifest_path, item['metadata'])
        protocol.require(digest(metadata_path) == manifest['files'][item['metadata']],
                         'calibration metadata changed')
        meta = q.read(metadata_path)
        bodies[name] = meta['body_sha256']
        inputs[name] = manifest['files'][item['input']]
        protocol.require(cell['status'] == 'complete', 'calibration cell failed')
        good, missing = paired_cell(cell, platform=manifest['platform'], seed=meta['seed'],
            family=item['family'], memory=memory, workers=int(worker))
        observations.extend(good)
        unavailable.extend(missing)
    return observations, unavailable, dict(platform=manifest['platform'],
        manifest_sha256=identity, result_sha256=digest(result_path),
        summary_sha256=digest(directory/'summary.json'), bodies=bodies,
        inputs=inputs,
        execution_sources=manifest['execution_sources'],
        revision=manifest['artifacts']['never']['revision'],
        build=manifest['artifacts']['never']['build'],
        artifact_sha256=manifest['files'][manifest['artifacts']['never']['path']])


def encode(value):
    if isinstance(value, Fraction):
        return dict(numerator=value.numerator, denominator=value.denominator)
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [encode(item) for item in value]
    return value


def main():
    import qualify as q
    from native_io import digest
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', action='append', nargs=2, metavar=('PLAN', 'RESULT_DIRECTORY'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    protocol.require(len(args.platform) == 2 and not args.output.exists(),
                     'require both platforms and a fresh output')
    observations, unavailable, bindings = [], [], []
    for plan, directory in args.platform:
        good, missing, binding = collect(Path(plan), Path(directory))
        observations.extend(good)
        unavailable.extend(missing)
        bindings.append(binding)
    protocol.require({binding['platform'] for binding in bindings} == {'Darwin', 'Linux'}
                     and bindings[0]['revision'] == bindings[1]['revision']
                     and bindings[0]['bodies'] == bindings[1]['bodies']
                     and bindings[0]['inputs'] == bindings[1]['inputs']
                     and bindings[0]['execution_sources'] == bindings[1]['execution_sources'],
                     'calibration platforms differ in source or generated inputs')
    result = freeze(observations, unavailable)
    result['bindings'] = bindings
    result['fitting_sources'] = {name: digest(Path(__file__).with_name(name+'.py')) for name in
        ('two_fe_policy_calibrate', 'two_fe_policy_fit', 'two_fe_policy_labels', 'two_fe_policy')}
    q.write(args.output, encode(result))
    print(result['status'])
    return int(result['status'] != 'fitted')


if __name__ == '__main__':
    raise SystemExit(main())
