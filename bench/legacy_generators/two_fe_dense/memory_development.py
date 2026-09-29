"""Bounded whole-fit memory diagnostic using a source-hashed worker override."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

import broad
import run


def main(a):
    a.root.mkdir(parents=True, exist_ok=False)
    folder = run.stage(a.root)
    worker = folder / 'measure.py'
    original = worker.read_text()
    marker = '    environment.update(thread_environment(threads))'
    assert original.count(marker) == 1
    worker.write_text(original.replace(marker,
        marker + '\n    environment["FEREG_MEMORY_LIMIT_BYTES"] = ' + repr(str(a.limit))))
    run.write(folder / 'source-manifest.json', run.controller_manifest(folder))
    commands = []
    for mode in ['fast', 'low', 'auto']:
        command = [sys.executable, str(Path(run.__file__)), str(a.root),
                   '--artifacts', str(a.artifacts), '--label', 'memory-' + mode,
                   '--variants', a.candidate, '--memory', mode, '--solver', 'auto',
                   '--design', a.design, '--n', '245760', '--widths', '50',
                   '--weight-kind', 'aw', '--weight-design', 'moderate',
                   '--cases', 'cluster2_cross_all', '--threads', '16', '--seed', '8675309',
                   '--repeats', '1', '--timeout', '120', '--keep-going']
        commands.append(command)
    plan = dict(schema='ols-whole-fit-memory-development-v1',
                scope='Science, native admission and failure behavior only; no performance claim.',
                imposed_available_bytes=a.limit, candidate=a.candidate, commands=commands,
                controller_sha256=broad.sha(Path(__file__)),
                original_worker_sha256=__import__('hashlib').sha256(original.encode()).hexdigest(),
                modified_worker_sha256=broad.sha(worker),
                generated_manifest_sha256=broad.sha(folder / 'source-manifest.json'))
    run.write(a.root / 'memory-plan.json', plan)
    records = []
    for command in commands:
        label = command[command.index('--label') + 1]
        with (a.root / (label + '.stdout.txt')).open('x') as stream:
            code = subprocess.call(command, stdout=stream, stderr=subprocess.STDOUT)
        completion = a.root / (label + '-complete.json')
        records.append(dict(label=label, process_returncode=code,
                            completion=json.loads(completion.read_text()) if completion.exists() else None))
        run.write(a.root / 'memory-progress.json', records)
        print(label, code, flush=True)
    run.write(a.root / 'memory-complete.json', dict(status='attempts_complete',
        planned=3, attempted=len(records), plan_sha256=broad.sha(a.root / 'memory-plan.json'),
        progress_sha256=broad.sha(a.root / 'memory-progress.json')))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('root', type=Path)
    p.add_argument('--artifacts', type=Path, required=True)
    p.add_argument('--candidate', required=True)
    p.add_argument('--design', choices=['bottleneck_ring', 'cycle'], default='cycle')
    p.add_argument('--limit', type=int, default=936870912,
                   help='available bytes before the normal 512 MiB minimum reserve')
    a = p.parse_args()
    assert a.limit > 0
    main(a)
