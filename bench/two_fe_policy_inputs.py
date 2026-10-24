#!/usr/bin/env python3
"""Prepare either frozen calibration inputs or the held-out development panel.

Uses maintained generators and independent references unchanged. Run from the
repository root with an explicitly supplied replay_large executable. No builds,
plugin calls, calibration fitting or timings occur here. There are deliberately
no family, seed, size or tolerance overrides. Partial failures are retained;
preparation never overwrites or resumes an output directory.
"""
from __future__ import annotations

import argparse
import copy
import importlib.metadata
import json
from pathlib import Path
import platform
import subprocess

import cmgschur_cases
import routing
import routing_cases
import routing_oracle
import two_fe_policy as policy
from native_io import ROOT, case_header, digest

HELD_OUT_SEEDS = (2026104201, 2026105201)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def source_files():
    """Preparation/reference modules plus Rust generator/serializer inputs.

    The Rust source snapshot and executable hash are separate observed bindings,
    not a claim that matching supplied hashes prove binary build provenance.
    """
    relative = [
        'bench/two_fe_policy_inputs.py', 'bench/two_fe_policy.py',
        'bench/two_fe_policy_development.py',
        'bench/routing_cases.py', 'bench/cmgschur_cases.py', 'bench/native_io.py',
        'bench/routing_oracle.py', 'bench/routing.py', 'bench/qualify.py',
        'scripts/replay_plugins.py', 'scripts/replay_psd.py',
        'scripts/replay_bias_reference.py', 'scripts/replay_cmg_polish.py',
        'crates/fereg-oracle/python/replay_ols_reference.py',
        'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
        'crates/fereg-core/Cargo.toml', 'crates/fereg-oracle/Cargo.toml',
        'crates/fereg-oracle/examples/replay_large.rs',
        'crates/fereg-oracle/examples/replay/wire.rs',
        'docs/two_fe_auto_gates.md',
    ]
    # The serializer consumes core metadata and support adapters. Bind complete
    # local crate source trees instead of guessing a narrower import closure.
    paths = [ROOT / name for name in relative]
    for folder in ('crates/fereg-core/src', 'crates/fereg-oracle/src',
                   'crates/fereg-oracle/tests/support'):
        paths.extend((ROOT / folder).rglob('*.rs'))
    return {str(path.relative_to(ROOT)): path for path in sorted(set(paths))}


def source_hashes():
    return {name: digest(path) for name, path in source_files().items()}


def stream_plan():
    """Family reuse within a seed is intentional; stages must remain disjoint."""
    require(policy.SEEDS == (2026102201, 2026103201), 'calibration seeds changed')
    require(policy.FAMILIES == ('dense', 'weak', 'high-degree-weak', 'repeated-cells',
                                'disconnected', 'weight-dynamic', 'wide-rhs', 'inactive-columns'),
            'calibration families changed')
    stages = {str(seed): {family: policy.streams(seed, family) for family in policy.FAMILIES}
              for seed in (*policy.SEEDS, *HELD_OUT_SEEDS)}
    unions = [{value for streams in families.values() for value in streams}
              for families in stages.values()]
    require(all(not left & right for i, left in enumerate(unions) for right in unions[i + 1:]),
            'calibration and held-out random streams collide')
    return stages


def _write(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write('\n')


def _fields(path):
    with Path(path).open('rb') as stream:
        shape, args = case_header(stream)
    require(args[0] == b'estimate-ols', 'calibration input must be OLS')
    pairs = [value.decode().split('=', 1) for value in args[1:]]
    require(all(len(pair) == 2 for pair in pairs), 'malformed request option')
    fields = dict(pairs)
    require(len(fields) == len(pairs), 'duplicate request option')
    require(fields.get('solver') == fields.get('preconditioner') == 'auto', 'source must be Auto/Auto')
    require(fields.get('memory') in ('fast', 'low'), 'source memory mode missing')
    require(not {'inject', 'break_marker_token'} & fields.keys(), 'source contains developer injection')
    return shape, args, fields


def _generate(generator, path, family, seed):
    """Same full-size branches/options as routing_cases.main, without imports."""
    if family == 'dense':
        options = dict(routing_cases.NATIVE_CASES[family])
        command = [str(generator), '--output', str(path), '--seed', str(seed),
                   '--solver', 'auto', '--vce', 'robust', '--graph', 'random-mobility',
                   '--tolerance', '1e-8']
        for key, value in options.items():
            command += ['--' + key, str(value)]
        # This is the existing native input serializer/generator, not an
        # estimation/plugin call. Failure propagates without replacement.
        subprocess.run(command, check=True, timeout=120, stdout=subprocess.DEVNULL)
        return {'generator_options': options, 'data_rng': 'fereg-oracle designs RNG'}
    if family in ('disconnected', 'weight-dynamic', 'inactive-columns'):
        columns, weights, count, intercept = routing_cases.extended_columns(family, seed)
        cmgschur_cases.write_case(generator, path, columns, include_intercept=intercept,
                                 effect_count=count, weights=weights)
        details = {'include_intercept': intercept, 'effect_count': count,
                   'weight_kind': 'unit' if weights is None else 'pw'}
    else:
        if family == 'high-degree-weak':
            kind = routing_cases.SAMPLED_CASES[family]
            sizes = cmgschur_cases.HIGH_DEGREE_SIZES[kind]
            columns = cmgschur_cases.high_degree_case(kind, *sizes, seed=seed)
            details = {'generator_options': {'kind': kind, 'workers': sizes[0],
                                             'firms': sizes[1], 'periods': sizes[2]}}
        else:
            kind, workers, firms, periods = routing_cases.SYNTHETIC_CASES[family]
            function = cmgschur_cases.additional_case if kind == 'wide' else cmgschur_cases.synthetic
            columns = function(kind, workers, firms, periods, seed=seed)
            details = {'generator_options': {'kind': kind, 'workers': workers,
                                             'firms': firms, 'periods': periods}}
        cmgschur_cases.write_case(generator, path, columns)
    return {**details, **cmgschur_cases.graph_summary(columns),
            'data_rng': 'NumPy default_rng',
            'serializer_template_seed': 20261013,
            'serializer_template_note': 'template numeric draws discarded; retained data streams listed separately'}


def _pair(fast, low, family):
    shape, arguments, fields = _fields(fast)
    require(fields['memory'] == 'fast', 'maintained generator did not emit Fast')
    routing_cases.copy_import(fast, low, low=True)
    low_shape, low_arguments, low_fields = _fields(low)
    require(shape == low_shape and low_fields['memory'] == 'low'
            and low_arguments == [b'memory=low' if arg.startswith(b'memory=') else arg for arg in arguments],
            'Fast/Low changed more than the memory request')
    fast_meta = routing_cases.inspect_case(fast, family)
    low_meta = routing_cases.inspect_case(low, family)
    require(fast_meta == low_meta and fast_meta['absorbed_terms'] == 2
            and fast_meta['categorical_intercepts'], 'Fast/Low model/body mismatch')
    return fast_meta


def _references(fast, low, family, body_hash):
    if family == 'inactive-columns':
        witness = routing_cases.inactive_case_oracle(fast)
        require(witness['regressors'] == 4, 'inactive witness width changed')
        return [{'kind': 'inactive'}, {'kind': 'inactive'}], {
            key: witness[key] for key in ('method', 'rank', 'components', 'regressors')}
    fast_ref = fast.with_suffix('.reference.json')
    reference = routing_oracle.write_reference(fast, fast_ref)
    require(reference['case_sha256'] == digest(fast) and reference['body_sha256'] == body_hash,
            'independent reference does not bind generated input')
    routing_oracle.load_reference(fast_ref, digest(fast), body_hash)
    # _pair proved exact body/model identity and the sole memory header change.
    # Rebind only the transport identity, retaining the actual independent arrays.
    low_ref = low.with_suffix('.reference.json')
    low_reference = copy.deepcopy(reference)
    low_reference['case_sha256'] = digest(low)
    low_reference['memory_pair_reference_sha256'] = digest(fast_ref)
    _write(low_ref, low_reference)
    routing_oracle.load_reference(low_ref, digest(low), body_hash)
    return [{'kind': 'routing', 'path': fast_ref.name},
            {'kind': 'routing', 'path': low_ref.name}], None


def prepare(output, *, generator, development=False):
    """Return a panel whose cases are qualify config entries relative to output.

    The caller supplies actual artifacts/harness/platform separately. The panel
    itself is preparation evidence, not a qualification configuration or result.
    """
    require(Path.cwd().resolve() == ROOT, 'run input preparation from the repository root')
    require(type(development) is bool, 'development stage must be explicit')
    if development:
        import two_fe_policy_development as stage
    else:
        stage = policy
    stages = stream_plan()
    output = routing_cases.scratch(Path(output))
    generator = Path(generator).resolve(strict=True)
    require(generator.is_file(), 'generator must be an existing executable file')
    require(not output.exists(), 'output already exists; no preparation retries or overwrite')
    before = source_hashes()
    binary_hash = digest(generator)
    source_identity = routing.identity(before)
    versions = {'python': platform.python_version(),
                'numpy': importlib.metadata.version('numpy'),
                'scipy': importlib.metadata.version('scipy')}
    output.mkdir(parents=True, exist_ok=False)
    cases, bodies = [], set()
    for seed in stage.SEEDS:
        for family in stage.FAMILIES:
            stem = f's{seed}-{family}'
            fast, low = (output / f'{stem}-{memory}.case' for memory in ('fast', 'low'))
            details = _generate(generator, fast, family, seed)
            inspected = _pair(fast, low, family)
            require(inspected['body_sha256'] not in bodies, 'generated calibration bodies collide')
            bodies.add(inspected['body_sha256'])
            references, inline_summary = _references(fast, low, family, inspected['body_sha256'])
            for path, memory, oracle in zip((fast, low), ('fast', 'low'), references):
                metadata = {**details, **inspected, 'schema': 1, 'family': family,
                            'coverage': [family], 'seed': seed, 'small': False,
                            'memory': memory, 'random_streams': policy.streams(seed, family),
                            'generator_sha256': binary_hash, 'generator_sources_sha256': source_identity,
                            'versions': versions, 'case_sha256': digest(path),
                            'evidence': 'prepared input with independent reference; no calibration or qualification'}
                if inline_summary is not None:
                    metadata['independent_reference_summary'] = inline_summary
                _write(path.with_suffix('.json'), metadata)
                cases.append({'id': f'{stem}-{memory}', 'family': family, 'input': path.name,
                              'oracle': oracle, 'memory_limit_bytes': None})
    input_count = 8 if development else 16
    require([case['id'] for case in cases] == list(stage.CASE_IDS) and len(bodies) == input_count,
            'frozen input/case coverage changed')
    # Reuse maintained panel guards with the metadata bindings q.plan will add.
    panel_cases = [{**case, 'metadata': str(Path(case['input']).with_suffix('.json'))} for case in cases]
    stage.validate_panel({'cases': panel_cases}, output / 'panel.json')
    require(digest(generator) == binary_hash and source_hashes() == before,
            'generator or preparation source drifted; retain failure, do not reuse partial output')
    value = {'schema': 1, 'kind': ('two-fe-gates-development-inputs-v1' if development else
                                 'two-fe-remaining-calibration-inputs-v1'),
             'status': 'prepared', 'seeds': list(stage.SEEDS), 'families': list(stage.FAMILIES),
             'input_count': input_count, 'case_count': 2 * input_count, 'cases': cases, 'random_streams': stages,
             'generator_sha256': binary_hash, 'generator_sources_sha256': source_identity,
             'sources': before, 'versions': versions,
             'files': {path.name: digest(path) for path in sorted(output.iterdir()) if path.is_file()}}
    _write(output / 'panel.json', value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True, help='new directory under target/tmp/TASK')
    parser.add_argument('--generator', type=Path, required=True, help='actual replay_large executable; not built here')
    parser.add_argument('--development', action='store_true', help='fixed seed 2026104201; final seed remains unavailable')
    args = parser.parse_args()
    value = prepare(args.output, generator=args.generator, development=args.development)
    print(f"{value['input_count']} frozen inputs / {value['case_count']} Fast/Low cases prepared; no native fits or timings")


if __name__ == '__main__':
    main()
