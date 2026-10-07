#!/usr/bin/env python3
"""Collect compact terminal SCC evidence without transferring inputs or row captures."""
from __future__ import annotations
import argparse
import hashlib
import io
import json
from pathlib import Path
import shlex
import subprocess
import tarfile


def accounting_records(text):
    result = []
    current = {}
    for line in text.splitlines():
        if line.startswith('===='):
            if current:
                result.append(current)
            current = {}
            continue
        pieces = line.strip().split(None, 1)
        if len(pieces) == 2:
            current[pieces[0]] = pieces[1]
    if current:
        result.append(current)
    required = {'jobnumber', 'taskid', 'hostname', 'qname', 'slots', 'failed',
                'exit_status', 'ru_wallclock', 'cpu', 'maxvmem', 'start_time', 'end_time'}
    if not result or any(not required <= r.keys() for r in result):
        raise ValueError('scheduler accounting is absent or structurally incomplete')
    return result


def remote(command):
    return subprocess.run(['ssh', 'scc', command], check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE).stdout


def collect(context_path, output):
    context = json.loads(context_path.read_text())
    root = context['remote_root']
    output.mkdir(parents=True, exist_ok=False)
    (output/'context.json').write_text(json.dumps(context, indent=2)+'\n')
    jobs = []
    for kind in ['setup', 'smoke', 'production']:
        job = str(context[kind+'_job'])
        if not job.isdigit():
            raise ValueError('invalid submitted job identity')
        raw = remote('qacct -j '+job).decode()
        (output/(kind+'-accounting.txt')).write_text(raw)
        records = accounting_records(raw)
        if any(r['jobnumber'] != job for r in records):
            raise ValueError('accounting identity mismatch')
        jobs.extend(records)
    # Final immutable inventory check uses the exact harness under test.
    verify = ('module purge; module load python3/3.12.4; cd '+shlex.quote(root+'/source')+
              '; python3 -c '+shlex.quote('from pathlib import Path; from bench.campaign import verify_runtime; '
                                        'verify_runtime(Path('+repr(root)+'))'))
    # campaign.py imports maintained sibling modules as a standalone entrypoint.
    verify = ('export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1; export PYTHONPATH='+
              shlex.quote(root+'/source/bench')+'; '+verify)
    remote(verify)
    files = ['manifest.json', 'input/source.json', 'setup.sh', 'job.sh', 'environment/runtime.json', 'environment/COMPLETE',
             'environment/reference-origin.json', 'environment/r-package-paths.json',
             'smoke/results.json', 'smoke/COMPLETE', 'logs/setup.sge', 'logs/smoke.sge',
             'logs/build.log', 'logs/clock-build.log', 'logs/harness-tests.log']
    files += [f'task-{i}/{name}' for i in range(1, 5) for name in ['results.json', 'COMPLETE']]
    listing = remote('find '+shlex.quote(root+'/logs')+" -maxdepth 1 -type f -name 'production.*.sge'").decode().splitlines()
    files.extend(str(Path(path).relative_to(root)) for path in listing)
    archive = remote('cd '+shlex.quote(root)+' && tar -cf - '+' '.join(shlex.quote(x) for x in files))
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:') as tar:
        for member in tar.getmembers():
            path = Path(member.name)
            if not member.isfile() or path.is_absolute() or '..' in path.parts:
                raise ValueError('unexpected compact evidence archive member')
            destination = output/path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(tar.extractfile(member).read())
    manifest = json.loads((output/'manifest.json').read_text())
    if json.loads((output/'input/source.json').read_text())['commit'] != context['source']:
        raise ValueError('collected source identity mismatch')
    sentinels = (output/'environment/COMPLETE').read_text().strip() == 'CAMPAIGN_SETUP_COMPLETE'
    sentinels &= 'CAMPAIGN_JOB_COMPLETE' in (output/'logs/smoke.sge').read_text()
    for task in range(1, manifest['policy']['bundles']+1):
        matches = [p for p in (output/'logs').glob('production.*.sge')
                   if f"CAMPAIGN_JOB_COMPLETE job={context['production_job']} task={task}" in p.read_text()]
        sentinels &= len(matches) == 1
    (output/'accounting.json').write_text(json.dumps(dict(jobs=jobs, sentinels_pass=bool(sentinels)), indent=2)+'\n')
    inventory = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in output.rglob('*') if p.is_file()}
    (output/'collection-sha256.json').write_text(json.dumps(inventory, indent=2)+'\n')
    # Import only after network work, keeping parsing usable without PDF libraries.
    from campaign_report import load_campaign
    load_campaign(output)
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('context', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(collect(args.context, args.output))
