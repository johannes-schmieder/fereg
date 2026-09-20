"""Execute the predeclared broad two-way development inventory without dropping cells.

Each label has an immutable plan and one attempt per cell. Resume skips recorded
attempts, including failures. Rechecks require a new label and preserve the old
plan/results. This controller does not declare scientific or timing acceptance.
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')


def expand(spec, kind):
    result = []
    for group in spec['primary_sets' if kind == 'primary' else 'native_controls']:
        default = dict(spec['default'], **{k: v for k, v in group.items() if k not in ('threads',)})
        axes = [group.get('designs', [default['design']]),
                group.get('n_values', [default['n']]), group['widths'], group['weights'],
                group['cases'], group.get('threads', [spec['default']['threads']]),
                group.get('weight_designs', [spec['weight_design']['primary']])]
        for design, n, k, weight, case, threads, distribution in itertools.product(*axes):
            cell = dict(group=group['name'], design=design, n=n, k=k, weight_kind=weight,
                        case=case, threads=threads, weight_design=distribution if weight != 'none' else 'none',
                        seed=default['seed'], shuffled=default.get('shuffled', False),
                        solver=default['solver'], memory=default['memory'],
                        effects=default.get('effects', ['g1', 'g2']))
            result.append(cell)
    assert len({json.dumps(cell, sort_keys=True) for cell in result}) == len(result)
    return result


def arguments(cell):
    args = ['--design', cell['design'], '--n', str(cell['n']), '--widths', str(cell['k']),
            '--weight-kind', cell['weight_kind'], '--weight-design', cell['weight_design'],
            '--cases', cell['case'], '--threads', str(cell['threads']), '--seed', str(cell['seed']),
            '--solver', cell['solver'], '--memory', cell['memory'], '--effects', *cell['effects']]
    if cell['shuffled']:
        args.append('--shuffled')
    return args


def main(a):
    spec = json.loads(a.manifest.read_text())
    kind = 'controls' if a.phase == 'controls' else 'primary'
    cells = expand(spec, kind)
    if a.groups:
        assert set(a.groups) <= {c['group'] for c in cells}
        cells = [c for c in cells if c['group'] in a.groups]
    repetitions = 1 if a.phase == 'screen' else 3 if kind == 'controls' else spec['acceptance']['paired_repetitions_per_block']
    variants = [a.baseline, a.candidate] if kind == 'controls' else [a.candidate, 'fixest']
    plan = dict(schema='two-fe-broad-execution-v1', label=a.label, phase=a.phase,
                inventory_kind=kind, repetitions=repetitions, variants=variants,
                manifest=spec, manifest_sha256=sha(a.manifest), controller_sha256=sha(Path(__file__)),
                cell_runner_sha256=sha(HERE / 'run.py'), cells=cells)
    if a.dry_run:
        print(json.dumps(plan, indent=2, sort_keys=True))
        return
    a.root.mkdir(parents=True, exist_ok=True)
    plan_path = a.root / (a.label + '-plan.json')
    progress_path = a.root / (a.label + '-progress.json')
    if plan_path.exists():
        assert json.loads(plan_path.read_text()) == plan, 'resume must retain the exact plan and controller'
    else:
        assert not progress_path.exists()
        write(plan_path, plan)
    progress = json.loads(progress_path.read_text()) if progress_path.exists() else []
    assert [c['index'] for c in progress] == list(range(len(progress)))
    for index, cell in enumerate(cells[len(progress):], start=len(progress)):
        label = f'{a.label}-c{index:03d}'
        command = [sys.executable, str(HERE / 'run.py'), str(a.root), '--artifacts', str(a.artifacts),
                   '--label', label, '--variants', *variants, '--repeats', str(repetitions),
                   '--record-reference-science-disagreement', '--record-reference-output-disagreement',
                   *arguments(cell)]
        output = a.root / (label + '.stdout.txt')
        # A crash between dispatch and progress must be inspected, never rerun
        # over existing measurement artifacts under the same identity.
        assert not output.exists(), f'unrecorded prior attempt requires inspection: {label}'
        with output.open('w') as stream:
            code = subprocess.call(command, stdout=stream, stderr=subprocess.STDOUT)
        complete = a.root / (label + '-complete.json')
        status = json.loads(complete.read_text())['status'] if complete.exists() else 'incomplete'
        progress.append(dict(index=index, label=label, cell=cell, process_returncode=code,
                             comparison_status=status, stdout_sha256=sha(output)))
        write(progress_path, progress)
        print(index + 1, '/', len(cells), label, code, status, flush=True)
    write(a.root / (a.label + '-complete.json'), dict(
        status='attempts_complete', attempted=len(progress), planned=len(cells),
        process_failures=sum(c['process_returncode'] != 0 for c in progress),
        comparisons={status:sum(c['comparison_status'] == status for c in progress)
                     for status in sorted({c['comparison_status'] for c in progress})},
        plan_sha256=sha(plan_path), progress_sha256=sha(progress_path)))
    print('BROAD_INVENTORY_ATTEMPTS_COMPLETE', flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('root', type=Path)
    p.add_argument('--artifacts', type=Path, required=True)
    p.add_argument('--manifest', type=Path, default=HERE / 'broad_milestone.json')
    p.add_argument('--candidate', required=True)
    p.add_argument('--baseline', default='baseline')
    p.add_argument('--label', required=True)
    p.add_argument('--phase', choices=['screen', 'block-a', 'block-b', 'controls'], required=True)
    p.add_argument('--groups', nargs='+')
    p.add_argument('--dry-run', action='store_true')
    main(p.parse_args())
