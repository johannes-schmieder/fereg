#!/usr/bin/env python3
"""Stage and submit one packed SCC campaign as a dependent SGE chain.

Run on an SCC login node (staging and submission only; no workloads).

  stage ROOT --source-tar T --source-json J --frozen-inputs F [--policy P]
        --export-prefix X [--plugin dist|build|candidate --candidate-package PKG]
        [--slots 28 --pe omp28 --cpu-type CPU]
  submit ROOT --reference-root R --comparator-root C [--after JOB] [--smoke-only]

`stage` creates a new run root with its input record, the launchers taken
from the source archive and a sha256sum-format manifest (paths relative to
the root); another whole-node size also sets the run's copy of job.sh,
whose hash the context records. `submit` verifies that manifest from the root, then submits setup,
smoke and the production array with -hold_jid, requiring a job id from every
qsub and recording each in context.json before the next submission.
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

LAUNCHERS = ['setup.sh', 'job.sh']
INPUTS = ['input/source.tar', 'input/source.json', 'input/frozen-inputs.json']


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def write_context(root, context):
    (root/'context.json').write_text(json.dumps(context, indent=2)+'\n')


def stage(root, source_tar, source_json, frozen_inputs, policy, prefix, slots, pe, cpu_type=None, plugin='dist',
          candidate_package=None):
    record = json.loads(Path(source_json).read_text())
    if (plugin == 'candidate') != bool(candidate_package):
        raise RuntimeError('a candidate plugin requires exactly its staged root package')
    if record.get('dirty') or sha256(source_tar) != record['archive_sha256']:
        raise RuntimeError('source archive does not match its clean record')
    root.mkdir(parents=False, exist_ok=False)
    (root/'input').mkdir()
    (root/'logs').mkdir()
    shutil.copyfile(source_tar, root/'input/source.tar')
    shutil.copyfile(source_json, root/'input/source.json')
    shutil.copyfile(frozen_inputs, root/'input/frozen-inputs.json')
    names = list(INPUTS)
    if policy:
        shutil.copyfile(policy, root/'input/policy.json')
        names.append('input/policy.json')
    with tarfile.open(root/'input/source.tar') as archive:
        for name in LAUNCHERS:
            (root/name).write_bytes(archive.extractfile('bench/scc/'+name).read())
            (root/name).chmod(0o755)
    if slots != 28 or cpu_type:
        # A hardware supplement reserves a different whole node; the launcher states it.
        job = (root/'job.sh').read_text()
        if '#$ -pe omp28 28\n' not in job or '#$ -l num_proc=28\n' not in job:
            raise RuntimeError('unexpected job.sh resource header')
        request = f'num_proc={slots}'+(f',cpu_type={cpu_type}' if cpu_type else '')
        (root/'job.sh').write_text(job.replace('#$ -pe omp28 28\n', f'#$ -pe {pe} {slots}\n')
                                   .replace('#$ -l num_proc=28\n', f'#$ -l {request}\n'))
    names += LAUNCHERS
    (root/'input/SHA256SUMS').write_text(''.join(f'{sha256(root/n)}  {n}\n' for n in names))
    write_context(root, dict(
        run_id=root.name, remote_root=str(root), source=record['commit'], source_sha256=record['archive_sha256'],
        host_observation_required=True, plugin=plugin, export_prefix=prefix,
        launcher_sha256={name: sha256(root/name) for name in LAUNCHERS},
        sampling_policy='One frozen schedule; no timing retries or interference exclusions; '
                        'complete reservation and passive observation; retain all failures.',
        resources=dict(setup=dict(slots=4, pe='omp', mem_per_core='4G', h_rt='02:00:00'),
                       smoke=dict(slots=slots, pe=pe, cpu_type=cpu_type),
                       production=dict(slots=slots, pe=pe, cpu_type=cpu_type, h_rt='12:00:00', mem_per_core='3G')),
        **(dict(candidate_package=str(candidate_package),
                candidate_plugin_sha256=sha256(Path(candidate_package)/'dist/fereg_linux.plugin'))
           if candidate_package else {})))
    print('CAMPAIGN_STAGED', root)


def qsub(arguments):
    output = subprocess.check_output(['qsub', '-terse', '-P', 'welfgr', '-l', 'no_gpu', *arguments], text=True).strip()
    match = re.fullmatch(r'(\d+)(?:\.[\d:-]+)?', output)
    if not match:
        raise RuntimeError('qsub did not return a job id: '+output)
    return match.group(1)


def submit(root, reference_root, comparator_root, after, smoke_only):
    context = json.loads((root/'context.json').read_text())
    if context.get('setup_job'):
        raise RuntimeError('refusing a duplicate submission')
    subprocess.run(['sha256sum', '-c', 'input/SHA256SUMS'], cwd=root, check=True)
    # With --after, setup's link step verifies the root once the build has ended.
    complete = Path(comparator_root)/'COMPLETE'
    if not after and (not complete.exists() or complete.read_text().strip() != 'CAMPAIGN_COMPARATORS_COMPLETE'):
        raise RuntimeError('comparator environment root is incomplete')
    whole = context['resources']['production']
    slots, pe = whole['slots'], whole['pe']
    hold = ['-hold_jid', after] if after else []
    common = (f'RUN_ROOT={root},REFERENCE_ROOT={reference_root},COMPARATOR_ROOT={comparator_root},'
              f"CAMPAIGN_PLUGIN={context['plugin']},CAMPAIGN_REUSE_ENVIRONMENT=1"
              +(f",CANDIDATE_PACKAGE={context['candidate_package']}" if context.get('candidate_package') else ''))
    context['reference_root'], context['comparator_root'] = str(reference_root), str(comparator_root)
    setup = qsub(['-N', 'frc_setup', '-pe', 'omp', '4', '-l', 'mem_per_core=4G,h_rt=02:00:00', *hold,
                  '-v', common, '-o', str(root/'logs/setup.sge'), str(root/'setup.sh')])
    context['setup_job'] = setup
    write_context(root, context)
    request = f'num_proc={slots},mem_per_core=3G,h_rt=12:00:00'+(f",cpu_type={whole['cpu_type']}" if whole.get('cpu_type') else '')
    resources = ['-pe', pe, str(slots), '-l', request]
    smoke = qsub(['-N', 'frc_smoke', *resources, '-hold_jid', setup, '-v',
                  f'RUN_ROOT={root},MODE=smoke,UPSTREAM_JOB={setup},EXPECTED_SLOTS={slots}',
                  '-o', str(root/'logs/smoke.sge'), str(root/'job.sh')])
    context['smoke_job'] = smoke
    write_context(root, context)
    jobs = dict(setup=setup, smoke=smoke)
    if not smoke_only:
        # A run-scoped policy override may set its own bundle count.
        with tarfile.open(root/'input/source.tar') as archive:
            bundles = json.load(archive.extractfile('bench/campaign.json'))['bundles']
        if (root/'input/policy.json').exists():
            bundles = json.loads((root/'input/policy.json').read_text()).get('bundles', bundles)
        production = qsub(['-N', 'frc_production', *resources, '-hold_jid', smoke, '-t', f'1-{bundles}', '-v',
                           f'RUN_ROOT={root},MODE=run,UPSTREAM_JOB={smoke},EXPECTED_SLOTS={slots}',
                           '-o', str(root/'logs/production.$TASK_ID.sge'), str(root/'job.sh')])
        context['production_job'] = production
        write_context(root, context)
        jobs['production'] = production
    print(json.dumps(jobs))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('stage')
    command.add_argument('root', type=Path)
    command.add_argument('--source-tar', type=Path, required=True)
    command.add_argument('--source-json', type=Path, required=True)
    command.add_argument('--frozen-inputs', type=Path, required=True)
    command.add_argument('--policy', type=Path)
    command.add_argument('--export-prefix', required=True)
    command.add_argument('--slots', type=int, default=28)
    command.add_argument('--pe', default='omp28')
    command.add_argument('--cpu-type', help='CPU model of a hardware supplement, for example 6737P')
    command.add_argument('--plugin', choices=['dist', 'build', 'candidate'], default='dist',
                         help="the archive's checksummed dist/ binary, a setup build, or a staged candidate")
    command.add_argument('--candidate-package', type=Path, help='staged candidate root package (with dist/, stata/)')
    command = commands.add_parser('submit')
    command.add_argument('root', type=Path)
    command.add_argument('--reference-root', type=Path, required=True)
    command.add_argument('--comparator-root', type=Path, required=True)
    command.add_argument('--after', help='job id that setup waits for (the environment build)')
    command.add_argument('--smoke-only', action='store_true')
    args = parser.parse_args()
    if args.command == 'stage':
        stage(args.root, args.source_tar, args.source_json, args.frozen_inputs, args.policy, args.export_prefix,
              args.slots, args.pe, args.cpu_type, args.plugin, args.candidate_package)
    else:
        submit(args.root, args.reference_root, args.comparator_root, args.after, args.smoke_only)
