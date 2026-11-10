#!/usr/bin/env python3
"""Pinned GLFixedEffectModels.jl and reghdfejl environments for the SCC campaign.

`build ENV_ROOT --origin ORIGIN` runs in a batch job. It verifies the pinned
comparator installation at ORIGIN (bench/reghdfejl/pins.json, bench/julia/ppml),
layers it into the new reused root ENV_ROOT, precompiles GLFixedEffectModels.jl
for the Stata/Julia bridge, checks both runtimes and fits through the bridge,
and writes ENV_ROOT/SHA256SUMS.

`link RUN_ROOT ENV_ROOT` runs in the campaign setup job. It verifies that
manifest and the campaign.json pins, then gives the run a private layer:
frozen trees are symlinked, while compilation caches and the files the bridge
loader compares by modification time are copied, so a measured process can
never write into the shared root.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
POLICY = json.loads((HERE/'campaign.json').read_text())
PINS = json.loads((HERE/'reghdfejl/pins.json').read_text())
RUNTIME = 'julia-'+PINS['julia_version']
CPU_TARGET = 'generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
# Frozen trees, hashed by the manifest and the run inventory; caches stay writable.
FROZEN = ['julia-ppml', 'julia-ppml-depot/packages', 'julia-ppml-depot/artifacts',
          'reghdfejl/references', 'reghdfejl/'+RUNTIME, 'reghdfejl/depot/packages',
          'reghdfejl/depot/artifacts', 'reghdfejl/depot/environments']
VERSIONS = ['julia-ppml-versions.json', 'reghdfejl/versions.json']


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def frozen_files(environment):
    """Every frozen file below ENVIRONMENT, following the layer's symlinks."""
    result = []
    for tree in FROZEN:
        top = environment/tree
        if not top.is_dir():
            raise RuntimeError('missing frozen tree: '+tree)
        for directory, _, names in os.walk(top, followlinks=True):
            result.extend(Path(directory)/name for name in names)
    return sorted(result)+[environment/name for name in VERSIONS]


def julia_ppml_pin():
    pin = POLICY['julia_ppml']
    return dict(julia=pin['julia'], GLFixedEffectModels=pin['GLFixedEffectModels'],
                FixedEffectModels=pin['FixedEffectModels'])


def verify_origin(origin):
    """The November 6 comparator installation matches every declared pin."""
    environment = origin/'environment'
    if (origin/'COMPLETE').read_text().strip() != 'COMPARATOR_SETUP_COMPLETE':
        raise RuntimeError('comparator installation is incomplete')
    actual = json.loads((environment/'reghdfejl/versions.json').read_text())
    for key, value in PINS.items():
        if actual[key] != value:
            raise RuntimeError('reghdfejl installation differs from its pin: '+key)
    references = environment/'reghdfejl/references'
    for name, item in PINS['files'].items():
        # The bridge plugin is rebuilt from the pinned source; the download is kept beside it.
        if sha256(references/(name+'.upstream' if name == 'jl.plugin' else name)) != item['sha256']:
            raise RuntimeError('pinned reghdfejl file differs: '+name)
    if sha256(references/'jl.plugin') != actual['bridge_plugin_sha256']:
        raise RuntimeError('rebuilt bridge plugin differs from its record')
    for name, expected in actual['manifests'].items():
        if sha256(environment/'reghdfejl'/name) != expected:
            raise RuntimeError('bridge environment manifest differs: '+name)
    for name in ['Project.toml', 'Manifest.toml']:
        if (HERE/'julia/ppml'/name).read_bytes() != (environment/'julia-ppml'/name).read_bytes():
            raise RuntimeError('GLFixedEffectModels.jl project differs: '+name)
    return actual


def link_tree(source, destination):
    destination.symlink_to(source.resolve(), target_is_directory=True)


def copy_tree(source, destination):
    # copy2 keeps modification times, which the bridge loader compares.
    shutil.copytree(source, destination, symlinks=False, copy_function=shutil.copy2)


def layer(source, target):
    """Create TARGET/{julia-ppml*,reghdfejl} over SOURCE (an environment directory)."""
    copy_tree(source/'julia-ppml', target/'julia-ppml')
    depot = target/'julia-ppml-depot'
    depot.mkdir()
    for name in ['packages', 'artifacts']:
        link_tree(source/'julia-ppml-depot'/name, depot/name)
    copy_tree(source/'julia-ppml-depot/compiled', depot/'compiled')
    bridge = target/'reghdfejl'
    bridge.mkdir()
    copy_tree(source/'reghdfejl/references', bridge/'references')
    link_tree(source/'reghdfejl'/RUNTIME, bridge/RUNTIME)
    depot = bridge/'depot'
    depot.mkdir()
    for name in ['packages', 'artifacts', 'registries']:
        if (source/'reghdfejl/depot'/name).is_dir():
            link_tree(source/'reghdfejl/depot'/name, depot/name)
    for name in ['environments', 'compiled']:
        copy_tree(source/'reghdfejl/depot'/name, depot/name)
    (bridge/'bin').mkdir()
    shim = bridge/'bin/julia'
    # jl asks `julia +CHANNEL` for libjulia; only the pinned runtime is admitted.
    shim.write_text('#!/bin/bash\nset -euo pipefail\nif [[ "${1:-}" == +* ]]; then\n'
                    f'  [[ "$1" == "+{PINS["julia_version"]}" ]] || exit 64\n  shift\nfi\n'
                    f'exec "{bridge/RUNTIME}/bin/julia" "$@"\n')
    shim.chmod(0o755)
    return bridge


def bridge_environment(bridge):
    return dict(os.environ, PATH=f"{bridge/'bin'}:{os.environ['PATH']}", JULIA_DEPOT_PATH=str(bridge/'depot'),
                JULIA_PKG_OFFLINE='true', JULIA_PKG_PRECOMPILE_AUTO='0', JULIA_CPU_TARGET=CPU_TARGET,
                OPENBLAS_NUM_THREADS='1')


def libjulia(bridge):
    return subprocess.run([str(bridge/'bin/julia'), '+'+PINS['julia_version'], '--startup-file=no', '-e',
                           'using Libdl; println(dlpath("libjulia"))'], env=bridge_environment(bridge),
                          check=True, capture_output=True, text=True).stdout.strip()


def bridge_versions(bridge, origin_record):
    pin = POLICY['reghdfejl']
    return dict(pin, pins_sha256=sha256(HERE/'reghdfejl/pins.json'),
                bridge_plugin_sha256=sha256(bridge/'references/jl.plugin'),
                bridge_compiler=origin_record['bridge_compiler'], origin_manifests=origin_record['manifests'],
                refs=str(bridge/'references'), bin=str(bridge/'bin'), depot=str(bridge/'depot'), lib=libjulia(bridge))


def compiled_inventory(depot):
    return {str(p.relative_to(depot)): (p.stat().st_size, p.stat().st_mtime_ns)
            for p in sorted((depot/'compiled').rglob('*')) if p.is_file()}


BRIDGE_TEST = '''version 19
clear all
set more off
set processors 2
sysdir set PLUS "{refs}"
sysdir set PERSONAL "{refs}"
adopath ++ "{refs}"
capture adopath - SITE
capture adopath - "."
jl start, threads(2) channel({julia})
_jl: @assert string(VERSION) == "{julia}";
_jl: @assert Threads.nthreads() == 2;
set obs 240
generate long g1 = mod(_n, 20) + 1
generate long g2 = mod(_n, 12) + 1
generate double x = sin(_n)
generate double y = exp(x/2 + cos(_n)/3 + g1/40 + g2/30)
reghdfejl y x, absorb(a=g1 b=g2) vce(robust) threads(2)
assert e(converged) == 1
drop a b
reghdfejl y x, absorb(g1 g2) vce(cluster g1) threads(2)
assert e(converged) == 1
reghdfejl y x, absorb(g1 g2) vce(robust) family(poisson) threads(2)
assert e(converged) == 1 & e(N) == 240
reghdfejl y x, absorb(g1 g2) vce(cluster g1) family(poisson) threads(2)
assert e(converged) == 1 & e(N_clust) == 20
* The pinned nonlinear route cannot save effects (save=:fe against save::Vector{{Symbol}}).
capture reghdfejl y x, absorb(a=g1 b=g2) vce(robust) family(poisson) threads(2)
assert _rc != 0
display "REGHDFEJL_PPML_SAVED_FE_UNSUPPORTED rc=" _rc
_jl: @assert string(pkgversion(FixedEffectModels)) == "{FixedEffectModels}";
_jl: @assert string(pkgversion(GLFixedEffectModels)) == "{GLFixedEffectModels}";
display "REGHDFEJL_BRIDGE_TEST_COMPLETE"
exit, clear
'''


def check_bridge(bridge, label):
    """Fit OLS and Poisson models through the bridge in a fresh Stata process."""
    directory = bridge/'setup'
    directory.mkdir(exist_ok=True)
    (directory/f'{label}.do').write_text(BRIDGE_TEST.format(refs=bridge/'references', **POLICY['reghdfejl']))
    subprocess.run([os.environ.get('STATA_BIN', 'stata-mp'), '-b', 'do', f'{label}.do'], cwd=directory,
                   env=bridge_environment(bridge), check=True)
    log = (directory/f'{label}.log').read_text(errors='replace')
    if '\nREGHDFEJL_BRIDGE_TEST_COMPLETE\n' not in log:
        raise RuntimeError(f'bridge test failed; inspect {directory}/{label}.log')


def check_julia_ppml(environment):
    """Load the pinned GLFixedEffectModels.jl arm environment and record its versions."""
    pin = julia_ppml_pin()
    script = ('using GLFixedEffectModels, GLM, Distributions, DataFrames, ReadStatTables, SHA, JSON; '
              'fem = only(m for (id, m) in Base.loaded_modules if id.name == "FixedEffectModels"); '
              'print(JSON.json(Dict("julia" => string(VERSION), '
              '"GLFixedEffectModels" => string(pkgversion(GLFixedEffectModels)), '
              '"FixedEffectModels" => string(pkgversion(fem)))))')
    env = dict(os.environ, JULIA_DEPOT_PATH=str(environment/'julia-ppml-depot'), JULIA_PKG_OFFLINE='true',
               JULIA_CPU_TARGET=CPU_TARGET)
    output = subprocess.run([os.environ.get('JULIA_BIN', 'julia'), '--startup-file=no',
                             f"--project={environment/'julia-ppml'}", '-e', script],
                            env=env, check=True, capture_output=True, text=True).stdout
    versions = json.loads(output)
    if versions != pin:
        raise RuntimeError(f'GLFixedEffectModels.jl environment {versions} differs from {pin}')
    return versions


def build(root, origin):
    record = verify_origin(origin)
    environment = root/'environment'
    environment.mkdir(parents=True)
    bridge = layer(origin/'environment', environment)
    # Precompile the nonlinear route for the bridge runtime; OLS packages are already compiled.
    pin = POLICY['reghdfejl']
    subprocess.run([str(bridge/'bin/julia'), '+'+pin['julia'], '--startup-file=no',
                    f"--project={bridge/'depot/environments/reghdfejl'}", '-e',
                    'using GLFixedEffectModels, GLM, FixedEffectModels, DataFrames, CategoricalArrays, Distributions, Vcov; '
                    f'@assert string(VERSION) == "{pin["julia"]}"; '
                    f'@assert string(pkgversion(GLFixedEffectModels)) == "{pin["GLFixedEffectModels"]}"; '
                    f'@assert string(pkgversion(FixedEffectModels)) == "{pin["FixedEffectModels"]}"'],
                   env=bridge_environment(bridge), check=True)
    versions = check_julia_ppml(environment)
    ppml_before = compiled_inventory(environment/'julia-ppml-depot')
    check_julia_ppml(environment)
    check_bridge(bridge, 'bridge-1')
    bridge_before = compiled_inventory(bridge/'depot')
    check_bridge(bridge, 'bridge-2')
    # A later process must find every cache it needs.
    if compiled_inventory(bridge/'depot') != bridge_before or compiled_inventory(environment/'julia-ppml-depot') != ppml_before:
        raise RuntimeError('a repeated process recompiled packages')
    (environment/'julia-ppml-versions.json').write_text(json.dumps(versions, indent=2)+'\n')
    (bridge/'versions.json').write_text(json.dumps(bridge_versions(bridge, record), indent=2)+'\n')
    lines = [f'{sha256(path)}  {path.relative_to(root)}' for path in frozen_files(environment)]
    (root/'SHA256SUMS').write_text('\n'.join(lines)+'\n')
    (root/'COMPLETE').write_text('CAMPAIGN_COMPARATORS_COMPLETE\n')
    print('CAMPAIGN_COMPARATORS_COMPLETE', len(lines), 'frozen files')


def link(run_root, root):
    if (root/'COMPLETE').read_text().strip() != 'CAMPAIGN_COMPARATORS_COMPLETE':
        raise RuntimeError('comparator environment root is incomplete')
    listed = {}
    for line in (root/'SHA256SUMS').read_text().splitlines():
        expected, name = line.split('  ', 1)
        if Path(name).is_absolute() or '..' in Path(name).parts or sha256(root/name) != expected:
            raise RuntimeError('comparator environment changed: '+name)
        listed[name] = expected
    if sorted(listed) != sorted(str(p.relative_to(root)) for p in frozen_files(root/'environment')):
        raise RuntimeError('comparator environment manifest is incomplete')
    source = root/'environment'
    if json.loads((source/'julia-ppml-versions.json').read_text()) != julia_ppml_pin():
        raise RuntimeError('GLFixedEffectModels.jl versions differ from campaign.json')
    recorded = json.loads((source/'reghdfejl/versions.json').read_text())
    if any(recorded[key] != value for key, value in POLICY['reghdfejl'].items()):
        raise RuntimeError('reghdfejl versions differ from campaign.json')
    if recorded['pins_sha256'] != sha256(HERE/'reghdfejl/pins.json'):
        raise RuntimeError('reghdfejl pins differ from the environment root')
    for name in ['Project.toml', 'Manifest.toml']:
        if (HERE/'julia/ppml'/name).read_bytes() != (source/'julia-ppml'/name).read_bytes():
            raise RuntimeError('GLFixedEffectModels.jl project differs: '+name)
    environment = run_root/'environment'
    bridge = layer(source, environment)
    shutil.copy2(source/'julia-ppml-versions.json', environment/'julia-ppml-versions.json')
    origin = dict(recorded, bridge_compiler=recorded['bridge_compiler'], manifests=recorded['origin_manifests'])
    versions = bridge_versions(bridge, origin)
    if versions['bridge_plugin_sha256'] != recorded['bridge_plugin_sha256']:
        raise RuntimeError('bridge plugin changed')
    (bridge/'versions.json').write_text(json.dumps(dict(versions, environment_root=str(root)), indent=2)+'\n')
    print('CAMPAIGN_COMPARATOR_LINK_COMPLETE', len(listed), 'frozen files')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('build')
    command.add_argument('root', type=Path)
    command.add_argument('--origin', type=Path, required=True)
    command = commands.add_parser('link')
    command.add_argument('run_root', type=Path)
    command.add_argument('root', type=Path)
    args = parser.parse_args()
    if args.command == 'build':
        build(args.root, args.origin)
    else:
        link(args.run_root, args.root)
