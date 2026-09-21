"""Whole-command route exploration before freezing any new automatic rule.

Each cell runs explicit solver families and auto in separate fresh processes.
The auto report retains Schur pilot diagnostics, so its Schur-CG path is never
identified with explicit symmetric CG. This controller records attempts only;
the independent graph oracle determines which successful routes are valid.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

import psutil

import broad

HERE = Path(__file__).resolve().parent
FAMILIES = ['auto', 'map', 'cg', 'lsmr', 'cmg', 'it']
GRAPHS = ['two_fe_dense', 'strong_d2', 'strong_d3', 'strong_d6', 'two_fe_sparse',
          'unbalanced', 'disconnected_blocks', 'weak_d3', 'bottleneck_ring', 'cycle']


def foreign_processes():
    owner = psutil.Process()
    ours = {owner.pid, *(p.pid for p in owner.children(recursive=True))}
    records = []
    for process in psutil.process_iter(['pid', 'name', 'create_time']):
        info = process.info
        if info['pid'] not in ours and info['name'] in ('stata-mp', 'R', 'Rscript', 'rustc', 'cargo'):
            # A worker may start after the descendants snapshot. Resolve its
            # live ancestry before treating our own new child as interference.
            try:
                parents = process.parents()
                if any(parent.pid == owner.pid for parent in parents):
                    continue
                info = dict(info, parent_pids=[parent.pid for parent in parents],
                            ancestry_status='resolved')
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                info = dict(info, ancestry_status='unavailable')
            records.append(info)
    return records


def invoke(command, output):
    # Other local tasks may start after the initial preflight. Preserve a
    # conservative overlap record and exclude such timings from route ranking.
    while foreign_processes():
        time.sleep(1)
    contamination = []
    started = time.time()
    with output.open('x') as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT)
        while process.poll() is None:
            foreign = foreign_processes()
            if foreign:
                contamination.append(dict(timestamp=time.time(), processes=foreign))
            time.sleep(0.25)
    return process.returncode, dict(started=started, finished=time.time(),
        status='no_detected_overlap' if not contamination else 'possible_overlap',
        sample_interval_seconds=0.25, competing_processes=contamination)


def cells(threads=16):
    return [dict(design=graph, n=983040 if graph == 'two_fe_dense' else 245760,
                 k=k, weight_kind=weight, weight_design='none' if weight == 'none' else 'moderate',
                 case=case, threads=threads, seed=8675309, shuffled=False,
                 solver='auto', memory='fast', effects=['g1', 'g2'])
            for graph in GRAPHS for k in [6, 50] for weight in ['none', 'aw']
            for case in ['robust_none', 'cluster2_cross_all']]


def main(a):
    inventory = cells(a.threads)
    families = a.families
    assert len(families) == len(set(families))
    indices = list(range(len(inventory))) if a.indices is None else a.indices
    assert len(indices) == len(set(indices)) and all(0 <= i < len(inventory) for i in indices)
    selected = [inventory[i] for i in indices]
    plan = dict(schema='ols-whole-command-route-exploration-v1', candidate=a.candidate,
                families=families, selected_indices=indices, cells=selected,
                repeats=a.repeats, timeout_seconds=a.timeout,
                controller_sha256=broad.sha(Path(__file__)), runner_sha256=broad.sha(HERE / 'run.py'),
                acceptance='Independent numerical audit before ranking; no policy or holdout acceptance.')
    if a.dry_run:
        print(json.dumps(plan, indent=2)); return
    a.root.mkdir(parents=True, exist_ok=False)
    broad.write(a.root / 'route-plan.json', plan)
    progress = []
    for index, cell in enumerate(selected):
        # Rotate family order between cells and repetitions. Each attempt has
        # its own immutable label; failures do not remove later families.
        for rep in range(a.repeats):
            offset = (index + rep) % len(families)
            for family in families[offset:] + families[:offset]:
                label = f'route-c{index:03d}-r{rep + 1}-{family}'
                request = dict(cell, solver=family)
                command = [sys.executable, str(HERE / 'run.py'), str(a.root),
                           '--artifacts', str(a.artifacts), '--label', label,
                           '--variants', a.candidate, '--repeats', '1', '--timeout', str(a.timeout),
                           '--keep-going', *broad.arguments(request)]
                output = a.root / (label + '.stdout.txt')
                code, isolation = invoke(command, output)
                completion = a.root / (label + '-complete.json')
                record = dict(index=index, original_index=indices[index], rep=rep + 1,
                              family=family, label=label, cell=request, command=command,
                              process_returncode=code, stdout_sha256=broad.sha(output),
                              timing_isolation=isolation,
                              completion=json.loads(completion.read_text()) if completion.exists() else None)
                progress.append(record)
                broad.write(a.root / 'route-progress.json', progress)
                print(len(progress), '/', len(selected) * len(families) * a.repeats,
                      label, code, flush=True)
    broad.write(a.root / 'route-complete.json', dict(status='attempts_complete',
                planned=len(selected) * len(families) * a.repeats, attempted=len(progress),
                process_failures=sum(p['process_returncode'] != 0 for p in progress),
                plan_sha256=broad.sha(a.root / 'route-plan.json'),
                progress_sha256=broad.sha(a.root / 'route-progress.json')))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path)
    p.add_argument('--artifacts', type=Path, required=True)
    p.add_argument('--candidate', required=True)
    p.add_argument('--indices', type=int, nargs='+')
    p.add_argument('--repeats', type=int, choices=[1, 3], default=1)
    p.add_argument('--families', choices=FAMILIES, nargs='+', default=FAMILIES)
    p.add_argument('--threads', type=int, choices=[1, 8, 16], default=16)
    p.add_argument('--timeout', type=int, default=60)
    p.add_argument('--dry-run', action='store_true')
    main(p.parse_args())
