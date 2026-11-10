#!/usr/bin/env python3
"""Stage and submit one revision campaign root as dependent SGE jobs.

Run on an SCC login node (staging and submission only; no workloads).

  stage ROOT --source-tar T --source-json J [--policy campaign_revision.json]
        --export-prefix X [--plugin dist|build|candidate --candidate-package PKG]
  submit ROOT --reference-root R --expanded-reference-root C --benchmark-input-root B
        [--validate-tasks "194 182"]
  production ROOT [--max-running 8]

`stage` creates a new root with its input record, the launchers taken from the
source archive and a sha256sum-format manifest (paths relative to the root).
Setup writes the frozen and real-panel inputs (revision_inputs.py), so they
are not staged. `submit` verifies that manifest from the root and submits setup,
then the whole-node smoke and, optionally, one whole-node job that runs the
listed manifest tasks back to back, each with -hold_jid on setup. `production`
requires the smoke's qualification receipt and submits the frozen array with
-tc. Every qsub must return a job id, which is recorded in context.json before
the next submission.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

LAUNCHERS = ['setup.sh', 'job.sh', 'campaign_tasks.sh']
WHOLE = ['-pe', 'omp28', '28', '-l', 'num_proc=28,mem_per_core=3G,h_rt=12:00:00']


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def write_context(root, context):
    (root/'context.json').write_text(json.dumps(context, indent=2)+'\n')


def stage(root, source_tar, source_json, policy, prefix, plugin='dist', candidate_package=None):
    record = json.loads(Path(source_json).read_text())
    if record.get('dirty') or sha256(source_tar) != record['archive_sha256']:
        raise RuntimeError('source archive does not match its clean record')
    if (plugin == 'candidate') != bool(candidate_package):
        raise RuntimeError('a candidate plugin requires exactly its staged root package')
    root.mkdir(parents=False, exist_ok=False)
    (root/'input').mkdir()
    (root/'logs').mkdir()
    shutil.copyfile(source_tar, root/'input/source.tar')
    shutil.copyfile(source_json, root/'input/source.json')
    names = ['input/source.tar', 'input/source.json']
    if policy:
        shutil.copyfile(policy, root/'input/policy.json')
        names.append('input/policy.json')
    with tarfile.open(root/'input/source.tar') as archive:
        for name in LAUNCHERS:
            (root/name).write_bytes(archive.extractfile('bench/scc/'+name).read())
            (root/name).chmod(0o755)
    names += LAUNCHERS
    (root/'input/SHA256SUMS').write_text(''.join(f'{sha256(root/n)}  {n}\n' for n in names))
    context = dict(
        run_id=root.name, remote_root=str(root), source=record['commit'], source_sha256=record['archive_sha256'],
        host_observation_required=True, plugin=plugin, export_prefix=prefix,
        launcher_sha256={name: sha256(root/name) for name in LAUNCHERS},
        evidence_policy='Retain every predetermined position, timeouts and foreign deviations; '
                        'no adverse resampling; raw row arrays are ephemeral node scratch.',
        resources=dict(setup=dict(slots=4, pe='omp', mem_per_core='4G', h_rt='02:00:00'),
                       smoke=dict(slots=28, pe='omp28', num_proc=28, mem_per_core='3G', h_rt='12:00:00'),
                       production=dict(slots=28, pe='omp28', num_proc=28, mem_per_core='3G', h_rt='12:00:00')))
    if candidate_package:
        context.update(candidate_package=str(candidate_package),
                       candidate_plugin_sha256=sha256(Path(candidate_package)/'dist/fereg_linux.plugin'))
    write_context(root, context)
    print('CAMPAIGN_STAGED', root)


def qsub(arguments):
    output = subprocess.check_output(['qsub', '-terse', '-P', 'welfgr', '-l', 'no_gpu', '-m', 'n', *arguments],
                                     text=True).strip()
    match = re.fullmatch(r'(\d+)(?:\.[\d:-]+)?', output)
    if not match:
        raise RuntimeError('qsub did not return a job id: '+output)
    return match.group(1)


def submit(root, reference_root, expanded_root, input_root, validate_tasks=None):
    context = json.loads((root/'context.json').read_text())
    if context.get('setup_job'):
        raise RuntimeError('refusing a duplicate submission')
    subprocess.run(['sha256sum', '-c', 'input/SHA256SUMS'], cwd=root, check=True)
    common = (f"RUN_ROOT={root},REFERENCE_ROOT={reference_root},CAMPAIGN_PLUGIN={context['plugin']},"
              f"CAMPAIGN_REUSE_ENVIRONMENT=1,CAMPAIGN_EXPANDED_COMPARATORS=1,"
              f"EXPANDED_REFERENCE_ROOT={expanded_root},BENCHMARK_INPUT_ROOT={input_root}"
              + (f",CANDIDATE_PACKAGE={context['candidate_package']}" if context.get('candidate_package') else ''))
    context.update(reference_root=str(reference_root), expanded_reference_root=str(expanded_root),
                   benchmark_input_root=str(input_root))
    setup = qsub(['-N', 'frv_setup', '-pe', 'omp', '4', '-l', 'mem_per_core=4G,h_rt=02:00:00',
                  '-v', common, '-o', str(root/'logs/setup.sge'), str(root/'setup.sh')])
    context['setup_job'] = setup
    write_context(root, context)
    smoke = qsub(['-N', 'frv_smoke', *WHOLE, '-hold_jid', setup, '-v',
                  f'RUN_ROOT={root},MODE=smoke,UPSTREAM_JOB={setup}', '-o', str(root/'logs/smoke.sge'), str(root/'job.sh')])
    context['smoke_job'] = smoke
    write_context(root, context)
    jobs = dict(setup=setup, smoke=smoke)
    if validate_tasks:
        tasks = ' '.join(str(int(t)) for t in validate_tasks.split())
        validation = qsub(['-N', 'frv_tasks', *WHOLE, '-hold_jid', setup, '-v',
                           f'RUN_ROOT={root},UPSTREAM_JOB={setup},TASKS={tasks}',
                           '-o', str(root/'logs/validation-tasks.sge'), str(root/'campaign_tasks.sh')])
        context['validation'] = dict(job=validation, tasks=tasks.split())
        write_context(root, context)
        jobs['validation'] = validation
    print(json.dumps(jobs))


def production(root, max_running):
    context = json.loads((root/'context.json').read_text())
    if context.get('production_job'):
        raise RuntimeError('refusing a duplicate production submission')
    receipt = json.loads((root/'qualification.json').read_text())
    manifest = json.loads((root/'manifest.json').read_text())
    if (not receipt.get('qualified') or receipt['manifest_sha256'] != sha256(root/'manifest.json')
            or receipt['smoke_results_sha256'] != sha256(root/'smoke/results.json')
            or not (root/'smoke/COMPLETE').exists()):
        raise RuntimeError('production requires a qualified smoke of this manifest')
    subprocess.run(['sha256sum', '-c', 'input/SHA256SUMS'], cwd=root, check=True)
    bundles = len(manifest['bundles'])
    job = qsub(['-N', 'frv_production', *WHOLE, '-hold_jid', context['smoke_job'], '-t', f'1-{bundles}',
                '-tc', str(max_running), '-v', f"RUN_ROOT={root},MODE=run,UPSTREAM_JOB={context['smoke_job']}",
                '-o', str(root/'logs/production.$TASK_ID.sge'), str(root/'job.sh')])
    context.update(production_job=job, production_tasks=list(range(1, bundles+1)), max_concurrent_allocations=max_running,
                   manifest_sha256=receipt['manifest_sha256'], qualification_sha256=sha256(root/'qualification.json'))
    context['resources']['production'].update(array=f'1-{bundles}', task_concurrency=max_running)
    write_context(root, context)
    print(json.dumps(dict(production=job, tasks=bundles, task_concurrency=max_running)))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('stage')
    command.add_argument('root', type=Path)
    command.add_argument('--source-tar', type=Path, required=True)
    command.add_argument('--source-json', type=Path, required=True)
    command.add_argument('--policy', type=Path, help='run policy override, for example bench/campaign_revision.json')
    command.add_argument('--export-prefix', required=True)
    command.add_argument('--plugin', choices=['dist', 'build', 'candidate'], default='dist',
                         help="the archive's checksummed dist/ binary, a setup build, or a staged candidate")
    command.add_argument('--candidate-package', type=Path, help='staged candidate root package (with dist/, stata/)')
    command = commands.add_parser('submit')
    command.add_argument('root', type=Path)
    command.add_argument('--reference-root', type=Path, required=True)
    command.add_argument('--expanded-reference-root', type=Path, required=True)
    command.add_argument('--benchmark-input-root', type=Path, required=True)
    command.add_argument('--validate-tasks', help='manifest tasks to run in one whole-node job after setup')
    command = commands.add_parser('production')
    command.add_argument('root', type=Path)
    command.add_argument('--max-running', type=int, default=8)
    args = parser.parse_args()
    if args.command == 'stage':
        stage(args.root, args.source_tar, args.source_json, args.policy, args.export_prefix, args.plugin,
              args.candidate_package)
    elif args.command == 'submit':
        submit(args.root, args.reference_root, args.expanded_reference_root, args.benchmark_input_root,
               args.validate_tasks)
    else:
        production(args.root, args.max_running)
