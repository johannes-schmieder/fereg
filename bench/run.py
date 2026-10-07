#!/usr/bin/env python3
"""Bounded local measurements of the installed fereg runtime."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time
import tomllib
from generate import generate, sha256
from compare import summarize

ROOT = Path(__file__).resolve().parents[1]

def load_suite(manifest=None):
    path = Path(manifest) if manifest else ROOT / "bench/suite.toml"
    return json.loads(path.read_text()) if path.suffix == ".json" else tomllib.loads(path.read_text())

def read_metadata(path):
    if not path.exists():
        return {}
    return dict(line.rstrip("\n").split("\t", 1) for line in path.read_text().splitlines() if "\t" in line)

def save_campaign(path, campaign):
    # A failed write leaves the previous checkpoint and partial temporary file.
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('x') as output:
        json.dump(campaign, output, indent=2)
        output.write('\n')
    temporary.replace(path)

def launch(command, directory, timeout):
    start = time.monotonic()
    with (directory / "process.log").open("w") as log:
        process = subprocess.Popen(command, cwd=directory, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            rc = process.wait(timeout=timeout)
            status = "ok" if rc == 0 else "error"
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            rc, status = None, "timeout"
    return {"process_status": status, "process_rc": rc, "wall_seconds": time.monotonic()-start}

def write_configuration(directory, settings):
    for value in settings.values():
        if any(char in str(value) for char in '\t\n\r"`$'):
            raise ValueError("benchmark configuration contains an unsafe Stata path/token")
    (directory / "config.tsv").write_text("".join(f"{key}\t{value}\n" for key, value in settings.items()))
    (directory / "config.do").write_text("".join(f'local {key} "{value}"\n' for key, value in settings.items()))

def selected_cells(suite, tier):
    return [{**suite['defaults'], **cell, 'rows': suite['tiers'][tier]['rows']}
            for cell in suite['cells']]


def artifact_identity():
    name = 'fereg_' + {'Darwin':'macos','Linux':'linux','Windows':'windows'}[platform.system()] + '.plugin'
    paths = [ROOT/'dist'/name, *sorted((ROOT/'stata').glob('*.ado')),
             *sorted((ROOT/'stata').glob('*.mo'))]
    return name, {str(p.relative_to(ROOT)):sha256(p) for p in paths}


def measure(args, suite, cell, threads, repetition, dataset, plugin, reference=False):
    label = 'reference' if reference else f't{threads}-{repetition}'
    directory = args.output/'commands'/cell['id']/label
    directory.mkdir(parents=True)
    settings = dict(source=ROOT, refs=directory, clock_plugin='', dataset=dataset,
                    output=directory, arm='fereg', model=cell['model'],
                    k=cell['regressors'], effects=cell['effects'], threads=threads,
                    deterministic=1, processor_cap=8, fixed_host_processors=1,
                    slopes=0, outputs=1, output_mode='combined', finite_factor=0,
                    weights=cell['weights'], vce=cell['vce'], plugin_name=plugin,
                    solver='auto', memory='auto', bias='', separation='',
                    tolerance=suite['protocol']['reference_tolerance'] if reference else '')
    write_configuration(directory, settings)
    (directory/'command.do').write_text(f'do "{ROOT / "bench/stata_runner.do"}" "{directory / "config.do"}"\n')
    result = launch([args.stata, '-b', 'do', str(directory/'command.do')], directory,
                    suite['tiers'][args.tier]['timeout_seconds'])
    metadata = read_metadata(directory/'metadata.tsv')
    result.update(cell=cell['id'], threads=threads, repetition=repetition,
                  reference=reference, directory=str(directory), metadata=metadata,
                  status=result['process_status'])
    if result['status'] == 'ok' and (metadata.get('complete') != '1' or metadata.get('rc') != '0'):
        result['status'] = 'error'
    print(f"{cell['id']} {label}: {result['status']}", flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tier', choices=['smoke','standard'], default='smoke')
    parser.add_argument('--stata', default=os.environ.get('STATA_BIN'))
    parser.add_argument('--output', type=Path, required=True, help='new output directory, normally under target/')
    args = parser.parse_args()
    if not args.stata: parser.error('set STATA_BIN or --stata')
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    suite = load_suite()
    plugin, identity = artifact_identity()
    campaign = dict(schema=1, tier=args.tier, suite=suite, artifacts=identity, datasets={}, records=[], complete=False)
    path = args.output/'run.json'
    save_campaign(path, campaign)
    try:
        for cell in selected_cells(suite,args.tier):
            dataset = args.output/'data'/(cell['id']+'.dta')
            campaign['datasets'][cell['id']] = generate(cell,suite['seed'],dataset)
            jobs = [(1, -1, True)] + [(t,i,False) for t in suite['tiers'][args.tier]['threads']
                     for i in range(suite['protocol']['repetitions']+1)]
            for threads,repetition,reference in jobs:
                if artifact_identity()[1] != identity: raise RuntimeError('runtime artifact changed')
                record = measure(args,suite,cell,threads,repetition,dataset,plugin,reference)
                campaign['records'].append(record)
                save_campaign(path,campaign)
                if record['status'] != 'ok': raise RuntimeError('command failed; capture retained')
            if sha256(dataset) != campaign['datasets'][cell['id']]['sha256']:
                raise RuntimeError('dataset changed')
        if artifact_identity()[1] != identity: raise RuntimeError('runtime artifact changed')
        campaign['complete'] = True
    finally:
        save_campaign(path,campaign)
    result = summarize(campaign)
    (args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
    return 0 if result['accuracy_pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
