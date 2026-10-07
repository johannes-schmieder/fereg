#!/usr/bin/env python3
"""Build the isolated command stopwatch into an ignored benchmark environment."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def build(output):
    if output.exists():
        raise ValueError('clock artifact already exists; builds never overwrite evidence')
    output.parent.mkdir(parents=True,exist_ok=True)
    system=platform.system()
    define={'Darwin':'APPLEMAC','Linux':'OPUNIX','Windows':'STWIN32'}[system]
    compiler='gcc' if system=='Windows' else 'cc'
    mode=['-bundle'] if system=='Darwin' else ['-shared']
    exports=[]
    export_file=None
    if system=='Darwin':
        export_file=ROOT/'plugin/exports_macos.txt'
        exports=['-Wl,-exported_symbols_list,'+str(export_file)]
    elif system=='Linux':
        export_file=ROOT/'plugin/exports_linux.map'
        exports=['-Wl,--version-script='+str(export_file)]
    command=[compiler,'-std=c11','-O2','-fPIC','-Wall','-Wextra','-Werror',
                    '-Wconversion','-Wshadow','-Wstrict-prototypes','-Wmissing-prototypes',
                    '-DSYSTEM='+define,'-I'+str(ROOT/'plugin'),*mode,*exports,
                    str(ROOT/'bench/clock.c'),str(ROOT/'plugin/stplugin.c'),'-o',str(output)]
    subprocess.run(command,check=True)
    if system=='Darwin':
        subprocess.run(['codesign','--force','--sign','-',str(output)],check=True)
    print(hashlib.sha256(output.read_bytes()).hexdigest(),output.name)
    output.with_suffix('.json').write_text(json.dumps({'platform':system,'binary_sha256':hashlib.sha256(output.read_bytes()).hexdigest(),
        'source_sha256':hashlib.sha256((ROOT/'bench/clock.c').read_bytes()).hexdigest(),
        'inputs':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in [ROOT/'bench/clock.c',ROOT/'plugin/stplugin.c',ROOT/'plugin/stplugin.h',*([export_file] if export_file else [])]},
        'compiler':subprocess.check_output([compiler,'--version'],text=True),'command':command},indent=2)+'\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,required=True)
    build(parser.parse_args().output.resolve())
