#!/usr/bin/env python3
"""Prepare fresh incremental routing inputs and bind a reviewed discovery stage.

Preparation runs generators and independent mathematics, never an estimator.
Only metadata for B/C is exposed to the discovery binder. Native prerequisites
remain allocation-local; neither this tool nor a file hash certifies science.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import sys

import routing
import routing_cases
import routing_discovery as discovery
import routing_oracle
import routing_parallel as parallel
import routing_run
from native_io import digest

SEEDS = {'many': {'A': 2026101711, 'B': 2026101712, 'C': 2026101713},
         'two': {'A': 2026101721, 'B': 2026101722, 'C': 2026101723}}
IMPORTS = ('no-fe', 'one-fe', 'slopes', 'low-saved')


def write(path, value):
    with Path(path).open('x') as out:
        json.dump(value, out, indent=2, sort_keys=True, allow_nan=False)
        out.write('\n')


def prepare(output, family, generator, imports):
    output = routing.scratch(output)
    generator, imports = (p.resolve(strict=True) for p in (generator, imports))
    tools = {str(p): digest(p) for p in (generator, imports, Path(__file__),
             Path(routing_cases.__file__), Path(routing_oracle.__file__))}
    output.mkdir(parents=True, exist_ok=False)
    panels = {}
    env = {**os.environ, **discovery.LIBRARY_LIMITS, 'VECLIB_MAXIMUM_THREADS': '1'}
    for stage, seed in SEEDS[family].items():
        supplied = output / ('imports-' + stage.lower())
        subprocess.run([str(imports), str(supplied), str(seed)], check=True, timeout=120, env=env)
        destination = output / stage.lower()
        command = [sys.executable, routing_cases.__file__, '--panel', stage, '--seed', str(seed),
                   '--output', str(destination), '--generator', str(generator), '--extended',
                   '--sampled', '--compact-it', '--low-saved', str(supplied / 'low-saved.case')]
        for name in IMPORTS[:3]:
            command += ['--source', f'{name}={supplied / (name + ".case")}']
        subprocess.run(command, check=True, timeout=2400, env=env)
        panels[stage] = routing.read_json(destination / 'panel.json')['cells']
    discovery.require(all(digest(Path(p)) == h for p, h in tools.items()), 'input generator changed')
    # Freeze all input identities before any native call. B/C values and targets
    # are not inspected here; only their generator-produced descriptors are read.
    write(output / 'panels.json', {'schema': 1, 'family': family, 'seeds': SEEDS[family],
          'panels': panels, 'generator_sha256': tools, 'native_calls': 0})


def references(inputs):
    """Fresh A targets only, with a separate 120-second limit per reference."""
    inputs = routing.scratch(inputs)
    description = routing.read_json(inputs / 'panels.json')
    output = inputs / 'references'
    output.mkdir(exist_ok=False)
    env = {**os.environ, **discovery.LIBRARY_LIMITS, 'VECLIB_MAXIMUM_THREADS': '1'}
    for cell in description['panels']['A'][::2]:
        if cell['family'] in (*IMPORTS, 'inactive-columns'):
            continue
        source = inputs / 'a' / cell['case_file']
        discovery.require(digest(source) == cell['case_sha256'], 'prepared A input changed')
        with (output / (source.stem + '.log')).open('x') as log:
            subprocess.run([sys.executable, routing_oracle.__file__, '--case', str(source),
                            '--output', str(output / (source.stem + '.json'))], env=env,
                           stdout=log, stderr=subprocess.STDOUT, check=True, timeout=125)


def manifest(config, prepared):
    family = prepared['family']
    candidate = 'incremental-' + family
    original, incumbent, current = (config[name]['sha256'] for name in
                                    ('baseline_plugin', 'incumbent_plugin', 'candidate_plugin'))
    arms = {'baseline': {'sha256': original, 'policy': None, 'role': 'required_reference'},
            'incumbent': {'sha256': incumbent, 'policy': None, 'role': 'required_reference'},
            candidate: {'sha256': current, 'policy': routing.policy(candidate), 'role': 'candidate'}}
    for name in sorted({a for cells in prepared['panels'].values() for c in cells for a in c['comparators']}):
        arm = {'sha256': current, 'role': 'optional_comparator', 'solver': name}
        if name == routing.COMPACT_IT:
            arm.update(solver='it', injection='it_coefficients')
        elif name.startswith('cmgschur-k'):
            arm.update(solver='cmgschur', cmgsamples=int(name[-1]))
        arms[name] = arm
    value = {'schema': 3, 'experiment_kind': 'incremental', 'warmups': 1,
        'failure_policy': copy.deepcopy(routing.INCREMENTAL_FAILURE_POLICY),
        'repetitions': dict(routing.REPETITIONS), 'arms': arms,
        'candidates': ['incumbent', candidate], 'protections': ['baseline', 'incumbent'],
        'harness_sha256': config['harness']['sha256'],
        'sampled_qualification': {'1': config['sampling_admission']['sha256']},
        'candidate_source_identity': config['candidate_source_identity'],
        'panels': prepared['panels'], 'prior_manifests': config['prior_manifests'],
        'incremental': config['incremental']}
    routing.validate_manifest(value)
    return value


def bind(config):
    inputs = Path(config['inputs']).resolve(strict=True)
    prepared = routing.read_json(inputs / 'panels.json')
    value = manifest(config, prepared)
    evidence = [config[k] for k in ('baseline_plugin', 'incumbent_plugin', 'candidate_plugin',
                'harness', 'resource_contract', 'sampling_admission')] + config['source_checks']
    for ref in evidence:
        discovery.checked(ref)
    output = routing.scratch(Path(config['output']))
    output.mkdir(parents=True, exist_ok=False)
    discovery.write(output / 'manifest.json', value)
    cases, witnesses = {}, {}
    for cell in value['panels']['A']:
        source = inputs / 'a' / cell['case_file']
        discovery.require(digest(source) == cell['case_sha256'], 'prepared A input changed')
        meta = routing.read_json(source.with_suffix('.json'))
        local = 'oracle' in meta or 'replay_reference' in meta or cell['family'] == 'inactive-columns'
        arms = list(dict.fromkeys(value['candidates'] + value['protections'] + cell['comparators']))
        witness = {'schema': 2, 'status': 'ready', 'manifest_sha256': routing.identity(value),
            'cell_id': cell['id'], 'case_sha256': cell['case_sha256'], 'body_sha256': cell['body_sha256'],
            'threads': cell['threads'], 'arms': {a: value['arms'][a] for a in arms},
            'resource_contract': config['resource_contract'], 'frozen_dependencies': evidence}
        if not local:
            target = inputs / 'references' / (source.stem + '.json')
            routing_oracle.load_reference(target, cell['case_sha256'], cell['body_sha256'])
            witness['independent_witness'] = discovery.reference(target)
        path = output / (cell['id'] + '.json')
        write(path, witness)
        routing_run.evidence(path, cell, value, arms, prerequisites_only=True, local_oracle=local)
        cases[cell['id']], witnesses[cell['id']] = str(source), str(path)
    plugins = {a: config['baseline_plugin' if a == 'baseline' else
                        'incumbent_plugin' if a == 'incumbent' else 'candidate_plugin']['path'] for a in value['arms']}
    parent = discovery.freeze({'manifest': str(output / 'manifest.json'), 'harness': config['harness']['path'],
        'plugins': plugins, 'cases': cases, 'witnesses': witnesses,
        'source_checks': [r['path'] for r in evidence], 'receipt_generator': parallel.isolation_host.__file__,
        'output_root': str(output / 'unused-parent')})
    discovery.write(output / 'parent.json', parent)
    plan = parallel.freeze({'schema': 3, 'discovery_plan': str(output / 'parent.json'),
        'output_root': str(output / 'discovery'), 'receipt_generator': parallel.isolation_host.__file__,
        'source_checks': [r['path'] for r in evidence],
        'scheduler_query_policy': discovery.SCHEDULER_QUERY_POLICY['kind']})
    discovery.write(output / 'plan.json', plan)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('prepare')
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--family', choices=SEEDS, required=True)
    p.add_argument('--generator', type=Path, required=True)
    p.add_argument('--imports', type=Path, required=True)
    p = sub.add_parser('references')
    p.add_argument('--inputs', type=Path, required=True)
    p = sub.add_parser('bind')
    p.add_argument('--config', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.output, args.family, args.generator, args.imports)
    elif args.command == 'references':
        references(args.inputs)
    else:
        bind(routing.read_json(args.config))


if __name__ == '__main__':
    main()
