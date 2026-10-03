#!/usr/bin/env python3
"""Instrument the bounded oracle/replay matrix separately from release artifacts."""
from __future__ import annotations
import json
import hashlib
import os
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'target/coverage-baseline'


def run(args, env, **kwargs):
    return subprocess.run(args, cwd=ROOT, env=env, check=True, **kwargs)


def main():
    if OUT.exists() and any(p.name != 'cargo' for p in OUT.iterdir()):
        raise SystemExit('coverage directory exists; choose a fresh campaign after preserving its summary')
    profiles = OUT / 'profiles'
    profiles.mkdir(parents=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(OUT / 'cargo'), RUST_TEST_THREADS='1',
               RUSTFLAGS='-C instrument-coverage', LLVM_PROFILE_FILE=str(profiles / '%m-%p.profraw'),
               FEREG_REPLAY_ROOT=str(OUT / 'replay'))
    with (OUT / 'checks.log').open('w') as log:
        # The full uninstrumented suite remains a separate required CI gate.
        # This campaign measures the bounded science/replay matrix, making its
        # unexercised production paths visible when choosing a simplification.
        run(['cargo', 'test', '-p', 'fereg-oracle', '--profile', 'ci', '--', '--include-ignored'], env, stdout=log, stderr=subprocess.STDOUT)
    objects = []
    for flags in [['-p','fereg-oracle']]:
        result = run(['cargo','test',*flags,'--profile','ci','--no-run','--message-format=json'], env, capture_output=True, text=True)
        for line in result.stdout.splitlines():
            message = json.loads(line)
            if message.get('executable') and message.get('profile',{}).get('test'):
                objects.append(Path(message['executable']))
    with (OUT / 'plugin.log').open('w') as log:
        run(['cargo','build','-p','fereg-plugin','--profile','ci'],env,stdout=log,stderr=subprocess.STDOUT)
        binary = OUT / 'fereg_instrumented.plugin'
        command = ['cc','-std=c11','-O2','-fPIC','-Iplugin','plugin/stplugin.c','plugin/stata_bridge.c',str(OUT/'cargo/ci/libfereg_plugin.a')]
        # rustc adds its matching profiler runtime when linking executables;
        # our Stata bundle is linked by cc, so add that same runtime explicitly.
        libdir = Path(run(['rustc','--print','target-libdir'],env,capture_output=True,text=True).stdout.strip())
        runtimes = list(libdir.glob('libprofiler_builtins-*.rlib'))
        if len(runtimes) != 1:
            raise SystemExit('expected exactly one profiler runtime in the pinned toolchain')
        if platform.system() == 'Linux':
            # ELF instrumentation does not itself pull this archive member;
            # retain its constructor so native plugin processes write profiles.
            command.append('-Wl,-u,__llvm_profile_runtime')
        command.append(str(runtimes[0]))
        if platform.system() == 'Darwin':
            command += ['-DSYSTEM=APPLEMAC','-bundle','-Wl,-exported_symbols_list,plugin/exports_macos.txt','-framework','Security','-framework','CoreFoundation','-framework','SystemConfiguration','-liconv','-lresolv']
        elif platform.system() == 'Linux':
            command += ['-DSYSTEM=OPUNIX','-shared','-Wl,--version-script=plugin/exports_linux.map','-ldl','-lpthread','-lm']
        else:
            raise SystemExit('coverage baseline currently requires a Unix host')
        run([*command,'-o',str(binary)],env,stdout=log,stderr=subprocess.STDOUT)
        # Compare instrumented runs to themselves; release exact-copy qualification
        # is a separate campaign, never inferred from instrumentation.
        native_profiles = OUT/'native-profiles'
        native_profiles.mkdir()
        replay_env = dict(env,LLVM_PROFILE_FILE=str(native_profiles/'%m-%p.profraw'))
        run(['bash','scripts/run_plugin_replay.sh',str(binary),str(binary)],replay_env,stdout=log,stderr=subprocess.STDOUT)
    objects.append(binary)
    sysroot = run(['rustc','--print','sysroot'],env,capture_output=True,text=True).stdout.strip()
    host = next(line.split(': ',1)[1] for line in run(['rustc','-vV'],env,capture_output=True,text=True).stdout.splitlines() if line.startswith('host: '))
    llvm = Path(sysroot)/'lib/rustlib'/host/'bin'
    raw = sorted(profiles.glob('*.profraw')) + sorted(native_profiles.glob('*.profraw'))
    if not raw:
        raise SystemExit('instrumentation produced no profiles')
    profile = OUT/'merged.profdata'
    run([str(llvm/'llvm-profdata'),'merge','-sparse',*[str(p) for p in raw],'-o',str(profile)],env)
    options = [f'-instr-profile={profile}', r'-ignore-filename-regex=(/rustc/|/\.cargo/|/vendor/)',
               *[f'-object={p}' for p in sorted(set(objects))]]
    with (OUT/'export.json').open('w') as export:
        run([str(llvm/'llvm-cov'),'export',*options],env,stdout=export)
    run([str(llvm/'llvm-cov'),'show',*options,'-format=html',f'-output-dir={OUT / "html"}'],env,stdout=subprocess.DEVNULL)
    identity = {'source_commit':run(['git','rev-parse','HEAD'],env,capture_output=True,text=True).stdout.strip(),
                'instrumentation':'Rust 1.85.1 -C instrument-coverage', 'profile':'ci',
                'platform':platform.system(), 'machine':platform.machine(), 'test_threads':1,
                'plugin_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
                'profile_files':len(raw), 'native_profile_files':len(list(native_profiles.glob('*.profraw'))),
                'objects':len(set(objects))}
    (OUT/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
    print(f'Coverage export: {OUT / "export.json"}')


if __name__ == '__main__': main()
