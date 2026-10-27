#!/usr/bin/env python3
"""Collect and validate aggregate Veneto campaign evidence; never fetch rows/FEs."""
from __future__ import annotations

import argparse
import copy
import hashlib
import io
import inspect
import json
from pathlib import Path
import shlex
import subprocess
import tarfile

from campaign_collect import accounting_records


def remote(command):
    return subprocess.check_output(['ssh', 'scc', command])


def audit_host_observations(root):
    """Replay retained scheduler observations without exporting private captures."""
    from qualify_scc import validate_census, validate_reservation
    reports = {}
    # One production task per scheduled panel; older collections scheduled both.
    manifest = root/'manifest.json'
    panels = json.loads(manifest.read_text()).get('panels', 'ls') if manifest.exists() else 'ls'
    for task in range(1, len(panels)+1):
        directory = root/f'host-task-{task}'
        report = json.loads((directory/'host.json').read_text())
        raw = (directory/'host-observations.jsonl').read_bytes()
        events = [json.loads(line) for line in raw.splitlines()]
        if not report['isolation_ok'] or report['errors'] or any(e['kind'] == 'error' for e in events):
            raise ValueError('host observation failed')
        reservations = [e for e in events if e['kind'] == 'reservation']
        if (len(reservations) != 2 or reservations[0]['monotonic_ns'] > report['started_monotonic_ns']
                or reservations[-1]['monotonic_ns'] < report['measurement_end_monotonic_ns']):
            raise ValueError('reservation does not bracket measurement')
        for event in reservations:
            validate_reservation(event['raw'], report['host'], 16)
        samples = {}
        for kind, gap, duration in [('process', 7.5, 7.5), ('census', 75, 20)]:
            selected = [e for e in events if e['kind'] == kind]
            starts = [e['monotonic_ns'] for e in selected]
            ending = report['measurement_end_monotonic_ns']
            if (len(selected) != report['samples'][kind]['count'] or not selected
                    or starts != sorted(set(starts)) or starts[0] > report['started_monotonic_ns']
                    or starts[-1] < ending):
                raise ValueError('host sample coverage differs')
            gaps = [(min(b, ending)-a)/1e9 for a, b in zip(starts, starts[1:])]
            if any(not 0 <= (e['finished_monotonic_ns']-e['monotonic_ns'])/1e9 <= duration for e in selected):
                raise ValueError('slow host sample')
            if any(value > gap for value in gaps):
                raise ValueError('missing host sample')
            for event in selected:
                if kind == 'census':
                    validate_census(event['raw']['job'], event['raw']['queues'], report['host'])
                elif event['competing_compute']:
                    raise ValueError('competing compute observed')
            samples[kind] = dict(count=len(selected), maximum_gap_seconds=max(gaps, default=0))
        reports[str(task)] = dict(host=report['host'], reservation_checks=2, samples=samples,
                                 observations_sha256=hashlib.sha256(raw).hexdigest(), passed=True)
    return reports


def audit_remote_hosts(context):
    # Inspection of small retained observer metadata only; no fitting or row/FE reads.
    script = ('import json, hashlib\nfrom pathlib import Path\n'+inspect.getsource(audit_host_observations)
              +'\nprint(json.dumps(audit_host_observations(Path("."))))\n')
    command = ('cd '+shlex.quote(context['remote_root'])
               +' && module load python3/3.12.4 && PYTHONPATH=source/bench python3 -')
    return json.loads(subprocess.check_output(['ssh', 'scc', command], input=script.encode()))


def load(root):
    context = json.loads((root/'context.json').read_text())
    manifest = json.loads((root/'manifest.json').read_text())
    jobs = json.loads((root/'accounting.json').read_text())
    panels = manifest.get('panels', {'long': None, 'short': None})
    tasks = range(1, len(panels)+1)
    expected = {(str(context['setup_job']), 'undefined'), (str(context['smoke_job']), 'undefined'),
                *((str(context['production_job']), str(task)) for task in tasks)}
    seen = set()
    for job in jobs:
        task = job['taskid'] if job['taskid'] not in ['0', 'NONE'] else 'undefined'
        key = job['jobnumber'], task
        if key in seen or key not in expected or job['failed'] != '0' or job['exit_status'] != '0':
            raise ValueError('unsuccessful, duplicate or unexpected scheduler accounting')
        slots = 28 if job['jobnumber'] == str(context['production_job']) else 4
        if int(job['slots']) != slots:
            raise ValueError('unexpected scheduler reservation')
        seen.add(key)
    if seen != expected:
        raise ValueError('missing terminal scheduler accounting')
    if json.loads((root/'input/source.json').read_text())['commit'] != context['source']:
        raise ValueError('source identity differs')
    for name, expected_hash in context['launcher_sha256'].items():
        if hashlib.sha256((root/name).read_bytes()).hexdigest() != expected_hash:
            raise ValueError('launcher changed: '+name)
    for name, sentinel in [('logs/setup.sge', 'PANEL_SETUP_COMPLETE'),
                           ('logs/smoke.sge', 'PANEL_JOB_COMPLETE')]:
        if sentinel not in (root/name).read_text():
            raise ValueError('missing application sentinel: '+name)
    if (root/'environment/COMPLETE').read_text().strip() != 'PANEL_SETUP_COMPLETE':
        raise ValueError('setup incomplete')
    smoke = json.loads((root/'smoke/results.json').read_text())
    if (not smoke.get('complete') or not smoke.get('native_gate_pass')
            or len(smoke['records']) != smoke.get('expected_fits', 8)):
        raise ValueError('real-data smoke incomplete')
    checks = [s for s in smoke['science'] if s['kind'] == 'dense-oracle' and s['arm'] == 'fereg']
    if len(checks) != len(panels) or not all(s['passed'] for s in checks):
        raise ValueError('native independent smoke failed')
    records, comparisons, science = [], [], []
    for task in tasks:
        label = f'task-{task}'
        result = json.loads((root/label/'results.json').read_text())
        host = json.loads((root/('host-'+label)/'host.json').read_text())
        accounted = next(j for j in jobs if j['jobnumber'] == str(context['production_job']) and j['taskid'] == str(task))
        if (not host.get('isolation_ok') or host.get('errors')
                or host['host']['job_id'] != str(context['production_job'])
                or host['host']['task_id'] != str(task)
                or host['host']['hostname'].split('.')[0] != accounted['hostname'].split('.')[0]):
            raise ValueError('whole-host observation/accounting mismatch: '+label)
        if (not result.get('complete') or not result.get('native_gate_pass')
                or str(result['job']) != str(context['production_job'])
                or str(result['task']) != str(task)
                or (root/label/'COMPLETE').read_text().strip() != 'PANEL_TASK_COMPLETE'):
            raise ValueError('production output incomplete: '+label)
        marker = f"PANEL_JOB_COMPLETE job={context['production_job']} task={task}"
        if marker not in (root/f'logs/production.{task}.sge').read_text():
            raise ValueError('production application sentinel absent: '+label)
        records.extend(result['records']); comparisons.extend(result['comparisons']); science.extend(result['science'])
    expected_positions = {(c['id'], arm, rep) for c in manifest['cells'] for arm in c['arms']
                          for rep in range(1, manifest['policy']['repetitions']+1)}
    positions = [(r['cell'], r['arm'], r['repetition']) for r in records]
    if len(positions) != len(set(positions)) or set(positions) != expected_positions:
        raise ValueError('campaign fit coverage differs from the frozen schedule')
    repetitions = manifest['policy']['repetitions']
    native = [r for r in records if r['arm'] == 'fereg']
    if len(native) != len(manifest['cells'])*repetitions or any(r['status'] != 'ok' or r.get('metadata', {}).get('rc') != '0'
                               or r['metadata'].get('complete') != '1' for r in native):
        raise ValueError('native application evidence failed')
    fixed = [s for s in science if s['kind'] == 'fixed-worker-exact']
    cross = [s for s in science if s['kind'] == 'cross-worker']
    if len(fixed) != len(manifest['cells'])*(repetitions-1) or not all(s['passed'] for s in fixed):
        raise ValueError('native fixed-worker exactness failed')
    if len(cross) != len(manifest['cells'])*repetitions or not all(s['eligible'] for s in cross):
        raise ValueError('native cross-worker scientific agreement failed')
    for panel in panels:
        audit = json.loads((root/'environment/input-audit.json').read_text())[panel]
        if not audit['exact_identifiers'] or audit['adjustment_max_error'] > 1e-12:
            raise ValueError('retained fevc input audit failed')
    return dict(manifest=manifest, context=context, accounting=jobs, smoke=smoke,
                records=records, comparisons=comparisons, science=science)


def collect(context_path, output):
    context = json.loads(context_path.read_text())
    run = context['remote_root']
    if output.exists():
        raise ValueError('collection directory must be new')
    output.mkdir(parents=True)
    (output/'context.json').write_text(json.dumps(context, indent=2)+'\n')
    jobs = []
    for stage in ['setup', 'smoke', 'production']:
        job = str(context[stage+'_job'])
        if not job.isdigit():
            raise ValueError('invalid submitted job')
        raw = remote('qacct -j '+job).decode()
        (output/(stage+'-accounting.txt')).write_text(raw)
        jobs.extend(accounting_records(raw))
    (output/'accounting.json').write_text(json.dumps(jobs, indent=2)+'\n')
    preparation = ['preparation/inputs.json', 'preparation/long-sample.json', 'preparation/short-sample.json']
    # A reused-input run retains its preparation records in the prior run.
    reused = context.get('previous_root')
    files = ['manifest.json', 'input/source.json', 'input/prior-panels.json', 'setup.sh', 'job.sh',
             *([] if reused else preparation), 'environment/runtime.json', 'environment/COMPLETE', 'environment/input-audit.json',
             'environment/reference-origin.json', 'environment/r-package-paths.json',
             'environment/pyfixest-freeze.txt', 'environment/pyfixest-within-freeze.txt',
             'logs/setup.sge', 'logs/smoke.sge', 'logs/harness-tests.log', 'logs/package-check.log',
             'logs/input-audit.log', 'smoke/results.json', 'smoke/COMPLETE']
    panels = json.loads(remote('cat '+shlex.quote(run+'/manifest.json'))).get('panels', 'ls')
    for task in range(1, len(panels)+1):
        files += [f'task-{task}/results.json', f'task-{task}/COMPLETE',
                  f'host-task-{task}/host.json', f'logs/production.{task}.sge']
    if reused:
        files += ['reuse.sge', 'logs/reuse.sge']
    archives = [(run, files)]+([(reused, preparation)] if reused else [])
    for directory, names in archives:
        raw = remote('cd '+shlex.quote(directory)+' && tar -cf - '+' '.join(shlex.quote(f) for f in names))
        with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
            for member in archive.getmembers():
                path = Path(member.name)
                if not member.isfile() or path.is_absolute() or '..' in path.parts:
                    raise ValueError('unsafe aggregate archive entry')
                destination = output/path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(archive.extractfile(member).read())
    result = load(output)
    (output/'host-validation.json').write_text(json.dumps(audit_remote_hosts(context), indent=2)+'\n')
    from campaign_report import summarize
    reporting = copy.deepcopy(result)
    reporting['manifest']['policy'].update(result['manifest']['policy']['scientific_tolerances'])
    summary = summarize(reporting)
    if not summary.loc[summary.arm == 'fereg', 'eligible'].all():
        raise ValueError('native summary eligibility failed; do not resample')
    summary.to_csv(output/'summary.csv', index=False)
    observations = []
    for record in result['records']:
        value = {k: record.get(k) for k in ['cell', 'arm', 'repetition', 'status', 'host', 'job', 'task',
                                            'estimator_tree_rss_bytes', 'memory_sweeps', 'error']}
        value['seconds'] = record.get('metadata', {}).get('seconds')
        observations.append(value)
    import pandas as pd
    pd.DataFrame(observations).to_csv(output/'observations.csv', index=False)
    (output/'science.json').write_text(json.dumps(dict(source=context['source'], science=result['science'],
                                                       comparisons=result['comparisons']), indent=2)+'\n')
    return summary


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('context', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(collect(args.context, args.output).to_string(index=False))
