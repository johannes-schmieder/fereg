#!/usr/bin/env python3
"""Create a bounded commit snapshot and an explicit SCC task manifest."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run import ROOT, selected_cells, thread_counts
from generate import sha256

PATHS = ['Cargo.toml','Cargo.lock','rust-toolchain.toml','clippy.toml','references.lock.toml',
         'crates','legacy','vendor','plugin','stata','scripts','ci/reference-bundles','tests/mock_spi',
         'bench','AGENTS.md','docs/SPEC.md','docs/ARCHITECTURE.md']


def prepare(root, tier, environment, revision, veneto, manifest=None, engine=None):
    if root.exists():
        raise ValueError('staging root already exists')
    commit=subprocess.check_output(['git','rev-parse',revision+'^{commit}'],cwd=ROOT,text=True).strip()
    paths=subprocess.check_output(['git','ls-tree','--name-only',commit,'--',*PATHS],cwd=ROOT,text=True).splitlines()
    root.mkdir(parents=True)
    source=root/'source.tar'
    with source.open('wb') as output:
        subprocess.run(['git','archive','--format=tar',commit,'--',*paths],cwd=ROOT,stdout=output,check=True)
    metadata={'commit':commit,'archive_sha256':sha256(source),'paths':paths}
    (root/'source.json').write_text(json.dumps(metadata,indent=2)+'\n')
    suite=tomllib.loads(subprocess.check_output(["git","show",commit+":bench/suite.toml"],cwd=ROOT,text=True))
    if manifest:
        relative=Path(manifest)
        if relative.is_absolute():
            relative=relative.relative_to(ROOT)
        if '..' in relative.parts or relative.parts[0] != 'bench' or relative.suffix != '.json':
            raise ValueError('program manifest must be a committed bench JSON file')
        suite=json.loads(subprocess.check_output(['git','show',commit+':'+relative.as_posix()],cwd=ROOT,text=True))
        (root/'suite.json').write_text(json.dumps(suite,indent=2)+'\n')
    if tier not in suite['tiers']:
        raise ValueError('tier is outside the committed manifest')
    selected_engine=engine or ('next' if manifest else 'legacy')
    if selected_engine not in {'next','legacy'}:
        raise ValueError('unknown native engine')
    (root/'policy.json').write_text(json.dumps({'engine':selected_engine,'program':bool(manifest)})+'\n')
    lock=tomllib.loads(subprocess.check_output(["git","show",commit+":references.lock.toml"],cwd=ROOT,text=True))
    pins={ref["name"]:ref for ref in lock["reference"]}
    if tier=='smoke':
        tasks=['all all']
    elif manifest:
        # Keep each complete ladder on the same whole node and avoid tiny arrays.
        tasks=[f'{cell["id"]} all' for cell in selected_cells(suite,tier)]
    else:
        tasks=[f'{cell["id"]} {threads}' for cell in selected_cells(suite,tier) for threads in thread_counts(suite,tier,cell)]
    (root/'tasks.tsv').write_text('\n'.join(tasks)+'\n')
    for name in ['fixest','ppmlhdfe']:
        archive=environment/'archives'/f'{name}.tar.gz'
        if sha256(archive)!=pins[name]['source_archive_sha256']:
            raise ValueError(f'local archive no longer matches the commit pin: {name}')
        shutil.copyfile(archive,root/f'{name}.tar.gz')
    if veneto:
        if not veneto.startswith('/projectnb/welfgr/') or any(c in veneto for c in '\n\r'):
            raise ValueError('Veneto input must be an explicit existing project path')
        (root/'veneto-path.txt').write_text(veneto+'\n')
    files=sorted(path for path in root.iterdir() if path.is_file())
    (root/'SHA256SUMS').write_text(''.join(f'{sha256(path)}  input/{path.name}\n' for path in files))
    print(commit, len(tasks), 'tasks', source.stat().st_size, 'source bytes')


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--root',type=Path,required=True)
    parser.add_argument('--tier',required=True)
    parser.add_argument('--manifest',type=Path)
    parser.add_argument('--engine',choices=['next','legacy'])
    parser.add_argument('--environment',type=Path,required=True)
    parser.add_argument('--revision',default='HEAD')
    parser.add_argument('--veneto-data')
    args=parser.parse_args()
    prepare(args.root,args.tier,args.environment,args.revision,args.veneto_data,args.manifest,args.engine)
