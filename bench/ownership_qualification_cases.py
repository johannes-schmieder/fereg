#!/usr/bin/env python3
"""Prepare the eight frozen ownership qualification additions, without estimation.

The original 42 requests are supplied separately and never rewritten here.
Uses the existing replay_large serializer and independent mathematical oracles.
No size/seed/tolerance overrides, overwrite, resume or replacement on failure.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import importlib.metadata
import json
from pathlib import Path
import platform
import signal

import numpy as np

import cmgschur_cases as generators
import routing_cases
import routing_oracle
from native_io import ROOT, case_header, digest

KIND = 'ownership-qualification-inputs-v1'
SEED = 2026106302
FAMILIES = ('ownership-nonmovers', 'ownership-hubs', 'ownership-inactive-zero',
            'ownership-cache-below', 'ownership-cache-at', 'ownership-cache-above',
            'ownership-repeated-large', 'ownership-weak-large')
ROWS = dict(zip(FAMILIES, (20000, 64000, 20000, 262143, 262144, 262145, 600000, 480000)))
WIDTHS = dict(zip(FAMILIES, (2, 2, 4, 16, 16, 16, 2, 2)))
LEVELS = dict(zip(FAMILIES, ((1000, 100), (1000, 100), (1000, 100),
                           (4096, 100), (4096, 100), (4096, 100), (5000, 100), (6000, 100))))
BOUNDARY = FAMILIES[3:6]
REFERENCE_SECONDS = 120


def require(condition, message):
    if not condition:
        raise ValueError(message)


def write_new(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def source_hashes():
    """Bind generator/reference closure and serializer source, separately from its binary."""
    names = ['bench/ownership_qualification_cases.py', 'bench/cmgschur_cases.py',
             'bench/routing_cases.py', 'bench/routing_oracle.py', 'bench/native_io.py',
             'scripts/replay_plugins.py', 'scripts/replay_psd.py',
             'scripts/replay_bias_reference.py', 'scripts/replay_cmg_polish.py',
             'crates/fereg-oracle/python/replay_ols_reference.py',
             'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
             'crates/fereg-core/Cargo.toml', 'crates/fereg-oracle/Cargo.toml',
             'docs/development_workflow.md', 'docs/SPEC.md']
    paths = [ROOT / name for name in names]
    for folder in ('crates/fereg-core/src', 'crates/fereg-oracle/src',
                   'crates/fereg-oracle/examples', 'crates/fereg-oracle/tests/support'):
        paths.extend((ROOT / folder).rglob('*.rs'))
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths))}


def specification(family):
    require(family in FAMILIES, 'unknown frozen qualification family')
    index = FAMILIES.index(family)
    seed = SEED + (3 if family in BOUNDARY else index)
    generator = ('additional_case' if family in (*BOUNDARY, FAMILIES[2]) else
                 'high_degree_case' if family == FAMILIES[1] else 'synthetic')
    kind = ('wide' if family in BOUNDARY else 'orthogonal' if family == FAMILIES[2]
            else 'd60-stress' if family == FAMILIES[1] else 'weak' if family == FAMILIES[7] else 'long')
    periods = (64 if family in (*BOUNDARY, FAMILIES[1]) else 30 if family == FAMILIES[6]
               else 80 if family == FAMILIES[7] else 20)
    transform = ('remove_last_row' if family == BOUNDARY[0] else
                 'append_last_row' if family == BOUNDARY[2] else
                 'first_100_workers_unchanged_rest_fixed_firm' if family == FAMILIES[0] else
                 'repeat_each_row_four_times' if family == FAMILIES[6] else 'identity')
    return dict(family=family, rows=ROWS[family], regressors=WIDTHS[family],
                workers=LEVELS[family][0], firms=LEVELS[family][1], periods=periods,
                generator=generator, generator_kind=kind, derived_seed=seed,
                random_streams=[seed, seed+1] if generator == 'additional_case' else [seed],
                row_transform=transform, include_intercept=family != FAMILIES[2],
                oracle_memory_budget_bytes=(2 if family in BOUNDARY else 1) << 30)


def boundary_columns(base, family):
    require(family in BOUNDARY, 'unknown boundary variant')
    if family == BOUNDARY[0]:
        return [column[:-1] for column in base]
    if family == BOUNDARY[2]:
        return [np.concatenate((column, column[-1:])) for column in base]
    return base


def generated_columns():
    """The boundary triplet shares one draw; no independent regeneration."""
    base = None
    for family in FAMILIES:
        spec = specification(family)
        if family in BOUNDARY:
            if base is None:
                require(family == BOUNDARY[0], 'boundary generation order changed')
                base = generators.additional_case('wide', 4096, 100, 64, SEED+3)
            values = boundary_columns(base, family)
        else:
            function = getattr(generators, spec['generator'])
            values = function(spec['generator_kind'], spec['workers'], spec['firms'],
                              spec['periods'], spec['derived_seed'])
            if family == FAMILIES[0]:
                values[-1][2000:] = np.repeat(np.arange(100, 1000) % 100, 20)
            elif family == FAMILIES[6]:
                values = [np.repeat(value, 4) for value in values]
        yield family, values
        if family == BOUNDARY[-1]:
            base = None


def validate_columns(values, spec):
    require(len(values) == spec['regressors']+3
            and all(np.asarray(v).shape == (spec['rows'],) and np.all(np.isfinite(v)) for v in values),
            'generated dimensions/nonfinite values differ from frozen shape')
    require([len(np.unique(v)) for v in values[-2:]] == [spec['workers'], spec['firms']],
            'generated categorical levels differ from frozen shape')


def frame_hints(source, target):
    """Set truthful validation hints only; canonical numeric/model bytes stay exact."""
    with Path(source).open('rb') as stream:
        shape, argv = case_header(stream)
        pairs = [arg.decode().split('=', 1) for arg in argv[1:]]
        require(argv[0] == b'estimate-ols' and all(len(p) == 2 for p in pairs), 'invalid OLS request')
        fields = dict(pairs)
        require(len(fields) == len(pairs) and not {'inject', 'break_marker_token'} & fields.keys(),
                'duplicate or injected request')
        require(all(fields.get(k) == v for k, v in {
            'solver': 'auto', 'preconditioner': 'auto', 'memory': 'fast', 'vce': 'robust',
            'cluster_term_count': '0', 'cluster_component_count': '0', 'account_cluster_nesting': '0',
            'absorb_term_count': '2', 'keep_singletons': '0'}.items())
            and fields.get('candidate_all') in ('0', '1') and fields.get('input_complete') in ('0', '1'),
            'canonical source model/hints differ')
        edits = {'candidate_all': '1', 'input_complete': '0'}
        amended = [argv[0], *[(k+'='+edits.get(k, v)).encode() for k, v in pairs]]
        body = hashlib.sha256()
        with Path(target).open('xb') as out:
            out.write(b'FEREG_REPLAY_V1\n'+(' '.join(map(str, shape))+'\n').encode())
            for arg in amended:
                out.write(str(len(arg)).encode()+b'\n'+arg+b'\n')
            for block in iter(lambda: stream.read(1 << 20), b''):
                body.update(block)
                out.write(block)
    inspected = routing_cases.inspect_case(Path(target))
    require(inspected['body_sha256'] == body.hexdigest(), 'request rewrite changed numeric/model body')
    return {k: dict(before=fields[k], after=v) for k, v in edits.items()}


@contextmanager
def reference_deadline():
    """Same 120-second wall limit as the maintained oracle CLI, with no retry."""
    require(signal.getitimer(signal.ITIMER_REAL) == (0.0, 0.0), 'reference cannot override an active alarm')
    previous = signal.getsignal(signal.SIGALRM)
    def expired(_signum, _frame):
        raise TimeoutError('independent reference exceeded its 120-second bound')
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(REFERENCE_SECONDS)
    try:
        yield
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, previous)


def reference(case, spec, body_hash):
    with reference_deadline():
        if spec['family'] == FAMILIES[2]:
            witness = routing_cases.inactive_case_oracle(case)
            require(witness['regressors'] == spec['regressors']
                    and len(witness['fitted']) == len(witness['residual']) == spec['rows'],
                    'inactive analytic witness dimensions differ')
            # The canonical analytic witness verifies exact within-cell pairing
            # and full finite rank. Retain its compact summary, not output arrays.
            return {'kind': 'inactive'}, dict(retained_rows=spec['rows'], full_finite_rank=True,
                **{k: witness[k] for k in ('method', 'rank', 'components', 'regressors')}), {}
        path = case.with_suffix('.oracle.json')
        routing_oracle.write_reference(case, path, memory_budget_bytes=spec['oracle_memory_budget_bytes'])
        result, files = routing_oracle.load_reference(path, digest(case), body_hash)
        require(result['rows'] == result['retained_rows'] == spec['rows']
                and result['regressors'] == spec['regressors']
                and result['include_intercept'] == spec['include_intercept']
                and result['certificate']['levels'] == [spec['workers'], spec['firms']]
                and result['certificate'].get('memory_budget_bytes', 1 << 30) == spec['oracle_memory_budget_bytes'],
                'independent retained rows/rank/model/allowance differ from frozen shape')
        return {'kind': 'routing', 'path': path.name}, dict(retained_rows=result['retained_rows'],
            regressors=result['regressors'], full_finite_rank=True,
            method='routing_oracle positive finite spectrum certificate',
            certificate=result['certificate']), files


def prepare(output, *, generator):
    output = routing_cases.scratch(Path(output))
    generator = Path(generator).resolve(strict=True)
    require(generator.is_file() and not output.exists(), 'generator missing or output already exists')
    sources, generator_hash = source_hashes(), digest(generator)
    versions = dict(python=platform.python_version(), numpy=importlib.metadata.version('numpy'),
                    scipy=importlib.metadata.version('scipy'))
    output.mkdir(parents=True, exist_ok=False)
    entries, files, bodies = [], {}, set()
    for family, values in generated_columns():
        require(family == FAMILIES[len(entries)], 'generation coverage/order changed')
        spec = specification(family)
        validate_columns(values, spec)
        case = output/(family+'.case')
        serialized = output/(family+'.serialized.case')
        generators.write_case(generator, serialized, values, include_intercept=spec['include_intercept'])
        edits = frame_hints(serialized, case)
        meta = routing_cases.inspect_case(case, family)
        require(meta['rows'] == spec['rows'] and meta['body_sha256'] not in bodies
                and meta['absorbed_terms'] == 2 and meta['categorical_intercepts'], 'serialized shape/body differs')
        # read_case checks every candidate bit, finite input and canonical source
        # binding, not merely the hint or filename. No weighted column is allowed.
        parsed = routing_oracle.read_case(case)
        require(parsed['x'].shape == (spec['rows'], spec['regressors'])
                and parsed['columns'] == spec['regressors']+10
                and parsed['include_intercept'] == spec['include_intercept']
                and np.all(parsed['weights'] == 1), 'serialized finite width/intercept/weights differ')
        del parsed
        bodies.add(meta['body_sha256'])
        oracle, witness, reference_files = reference(case, spec, meta['body_sha256'])
        meta.update(schema=1, family=family, seed=SEED, small=False, **specification_metadata(spec),
                    candidate_all_verified=True, input_complete=False, request_hint_edits=edits,
                    case_sha256=digest(case), generator_sha256=generator_hash, generator_sources=sources,
                    versions=versions, independent_reference_summary=witness,
                    evidence='Prepared input and independent reference; no native estimation or timing.')
        metadata = case.with_suffix('.json')
        write_new(metadata, meta)
        for path in (case, metadata, *reference_files):
            files[path.name] = digest(path)
        entries.append(dict(id=family, family=family, input=case.name, metadata=metadata.name,
                            oracle=oracle, memory_limit_bytes=None))
        serialized.unlink()  # Successful framing/reference; failures retain their source.
    require(len(entries) == 8 and len(bodies) == 8, 'incomplete added input panel')
    require(source_hashes() == sources and digest(generator) == generator_hash, 'preparation source/binary drift')
    panel = dict(schema=1, kind=KIND, seed=SEED, families=list(FAMILIES), cases=entries, files=files,
                 generator_sha256=generator_hash, generator_sources=sources, versions=versions,
                 reference_timeout_s=REFERENCE_SECONDS, native_calls=0,
                 evidence='Eight added inputs only; original 42 requests remain separately bound.')
    write_new(output/'panel.json', panel)
    return panel


def specification_metadata(spec):
    return {k: v for k, v in spec.items() if k not in ('family', 'rows')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generator', type=Path, required=True, help='existing canonical replay_large serializer')
    parser.add_argument('--output', type=Path, required=True, help='new directory under target/tmp/TASK')
    args = parser.parse_args()
    panel = prepare(args.output, generator=args.generator)
    print(f"{len(panel['cases'])} added cases prepared; zero native estimations")


if __name__ == '__main__':
    main()
