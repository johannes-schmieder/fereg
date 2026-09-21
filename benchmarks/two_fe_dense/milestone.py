"""Expand the approved September 21 graph coverage; retain every attempted cell.

This is a development controller. Completion describes attempts, not scientific
or timing acceptance; reference discrepancies and failures remain visible.
"""
import argparse
import itertools
import json
from pathlib import Path
import subprocess
import sys
import broad

HERE = Path(__file__).resolve().parent
MANIFEST = HERE.parent / 'manifests/ols_optimization_20260921_v1.json'
REQUESTS = {
    'robust_no_output': 'robust_none',
    'cluster2_all_outputs': 'cluster2_cross_all',
    'cluster_cross_saved': 'cluster_cross_saved',
    'cluster2_nested_no_output': 'cluster2_nested_none',
}


def expand(spec, phase):
    default = spec['defaults']
    groups = []
    if phase == 'small':
        small = spec['small_science']
        requests = [vce + '_' + output for vce, output in itertools.product(small['inference'], small['outputs'])]
        groups = [dict(graphs=spec['graphs'], n=960, widths=small['widths'], requests=requests)]
    else:
        groups = spec['new_large_timing']
    result = []
    for group in groups:
        for graph, k, weight, request in itertools.product(group['graphs'], group['widths'], default['weights'], group['requests']):
            case = request if phase == 'small' else REQUESTS[request]
            if weight == 'pw' and case.startswith('iid_'):
                continue
            result.append(dict(design='two_fe_dense' if graph == 'shuffled_dense' else graph,
                shuffled=graph == 'shuffled_dense', n=(7680 if graph == 'weak_d3' else 1920) if phase == 'small' else group['n'], k=k, weight_kind=weight,
                weight_design='none' if weight == 'none' else 'moderate', case=case,
                threads=default['threads'], seed=default['seed'], solver=default['solver'],
                memory=default['memory'], effects=['g1', 'g2']))
    assert len({json.dumps(c, sort_keys=True) for c in result}) == len(result)
    return result


def main(a):
    spec = json.loads(MANIFEST.read_text())
    for name in ('primary', 'holdout'):
        path = HERE.parents[1] / spec['protection'][name + '_manifest']
        assert broad.sha(path) == spec['protection'][name + '_manifest_sha256']
    cells = expand(spec, a.phase)
    if a.indices:
        assert len(a.indices) == len(set(a.indices))
        cells = [cells[i] for i in a.indices]
    repeats = 1 if a.phase == 'small' else 3 if a.phase == 'screen' else 5
    plan = dict(schema='ols-graph-coverage-execution-v1', phase=a.phase, label=a.label,
        manifest=spec, manifest_sha256=broad.sha(MANIFEST), cells=cells, selected_indices=a.indices, repeats=repeats,
        baseline=a.baseline, candidate=a.candidate, controller_sha256=broad.sha(Path(__file__)),
        runner_sha256=broad.sha(HERE / 'run.py'), keep_going=a.keep_going)
    if a.dry_run:
        print(json.dumps(plan, indent=2)); return
    a.root.mkdir(parents=True, exist_ok=True)
    plan_path = a.root / (a.label + '-plan.json')
    progress_path = a.root / (a.label + '-progress.json')
    if plan_path.exists():
        assert json.loads(plan_path.read_text()) == plan, 'resume requires the exact recorded plan'
    else:
        assert not progress_path.exists(); broad.write(plan_path, plan)
    progress = json.loads(progress_path.read_text()) if progress_path.exists() else []
    assert [item['index'] for item in progress] == list(range(len(progress)))
    for index, cell in enumerate(cells[len(progress):], start=len(progress)):
        label = a.label + f'-c{index:04d}'
        variants = [a.baseline, a.candidate] if cell['weight_kind'] == 'fw' else [a.baseline, a.candidate, 'fixest']
        command = [sys.executable, str(HERE / 'run.py'), str(a.root), '--artifacts', str(a.artifacts),
            '--label', label, '--variants', *variants, '--repeats', str(repeats),
            '--record-reference-science-disagreement', '--record-reference-output-disagreement',
            *broad.arguments(cell)]
        if a.phase == 'small': command.append('--oracle')
        if a.keep_going: command.append('--keep-going')
        output = a.root / (label + '.stdout.txt')
        assert not output.exists(), 'unrecorded prior dispatch requires inspection: ' + label
        with output.open('w') as stream:
            code = subprocess.call(command, stdout=stream, stderr=subprocess.STDOUT)
        complete = a.root / (label + '-complete.json')
        status = json.loads(complete.read_text())['status'] if complete.exists() else 'incomplete'
        progress.append(dict(index=index, cell=cell, label=label, process_returncode=code,
            comparison_status=status, stdout_sha256=broad.sha(output)))
        broad.write(progress_path, progress)
        print(index + 1, '/', len(cells), label, code, status, flush=True)
    broad.write(a.root / (a.label + '-complete.json'), dict(status='attempts_complete',
        planned=len(cells), attempted=len(progress), process_failures=sum(p['process_returncode'] != 0 for p in progress),
        plan_sha256=broad.sha(plan_path), progress_sha256=broad.sha(progress_path)))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path); p.add_argument('--artifacts', type=Path, required=True)
    p.add_argument('--candidate', required=True); p.add_argument('--baseline', default='baseline')
    p.add_argument('--label', required=True); p.add_argument('--phase', choices=['small', 'screen', 'block-a', 'block-b'], required=True)
    p.add_argument('--indices', type=int, nargs='+'); p.add_argument('--dry-run', action='store_true')
    p.add_argument('--keep-going', action='store_true')
    main(p.parse_args())
