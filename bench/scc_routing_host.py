#!/usr/bin/env python3
"""Collect and independently replay SCC reservation evidence for one frozen A screen.

Scheduler reservation and sampled competing compute are separate evidence.
Ordinary OS activity remains in the samples. No process is paused or terminated,
and this tool never examines solve times or changes the collection schedule.
"""
from __future__ import annotations

import argparse
import base64
import copy
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
import xml.etree.ElementTree as ET

import block_host
import routing
import routing_discovery as discovery

KINDS = discovery.EVIDENCE
PS_FIELDS = 'pid=,ppid=,lstart=,state=,time=,pcpu=,rss=,comm='
PS_ARGS = 'pid=,lstart=,args='
SLOTS = 28


def require(condition, message):
    if not condition:
        raise ValueError(message)


def hostname(value):
    parts = value.split('.')
    require(len(parts) == 1 or parts[1:] == ['scc', 'bu', 'edu'], 'unexpected SCC host domain')
    return parts[0]


def command(argv, timeout=10):
    result = subprocess.run(argv, capture_output=True, text=True, timeout=timeout,
                            env={**os.environ, 'LC_ALL': 'C', 'TZ': 'UTC'})
    require(result.returncode == 0, f'evidence command failed: {argv}: {result.stderr}')
    return {'argv': argv, 'stdout': result.stdout, 'stderr': result.stderr,
            'returncode': result.returncode, 'locale': 'C', 'timezone': 'UTC'}


def output(record, argv):
    require(record.get('argv') == argv and record.get('returncode') == 0
            and record.get('locale') == 'C' and record.get('timezone') == 'UTC',
            'raw command identity/status differs')
    require(isinstance(record.get('stdout'), str) and record['stdout'], 'empty raw command output')
    return record['stdout']


def append(path, value):
    with path.open('a') as stream:
        stream.write(json.dumps(value, sort_keys=True, allow_nan=False) + '\n')


def rows(path):
    with path.open() as stream:
        for line in stream:
            require(line.endswith('\n'), 'partial raw evidence row')
            yield json.loads(line)


def fields(text, colon=False):
    values = {}
    text = text.replace('\\\n', '')
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith('#') or line in {'{', '}'} or set(line) == {'='}:
            continue
        pair = line.split(':', 1) if colon else line.split(None, 1)
        if len(pair) == 2:
            key, value = (part.strip() for part in pair)
            require(key not in values, 'duplicate raw configuration field: ' + key)
            values[key] = value
    return values


def cpu_set(text):
    values = set()
    for part in text.strip().split(','):
        bounds = part.split('-')
        require(1 <= len(bounds) <= 2 and all(s.isdigit() for s in bounds), 'invalid CPU range')
        low, high = int(bounds[0]), int(bounds[-1])
        require(0 <= low <= high <= 65535, 'invalid CPU range bound')
        values.update(range(low, high + 1))
    return sorted(values)


def policy(plan):
    value = copy.deepcopy(block_host.POLICY)
    native = Path(plan['harness']).name
    value['executables'] += sorted({native, native[:15], 'actual_plugin_loader_test',
                                    'actual_plugin_l'} - set(value['executables']))
    return value


def check_sources(plan):
    paths = [Path(__file__), Path(block_host.__file__), Path(discovery.__file__), Path(routing.__file__)]
    if 'round_contract' in plan:
        paths.append(Path(__file__).with_name('routing_parallel.py'))
    for path in paths:
        ref = discovery.reference(path)
        require(plan['frozen_files'].get(ref['path']) == ref['sha256'], 'unfrozen isolation tool dependency')
    require(plan['receipt_generator'] == discovery.reference(__file__), 'wrong receipt generator')


def round_contract(plan, collection=None):
    if 'round_contract' not in plan:
        require(collection is None or 'round_contract' not in collection, 'unexpected collection round contract')
        return None
    value = plan['round_contract']
    partitioned = 'discovery_partition' in plan
    if partitioned:
        discovery.execution_spec(plan)
    require(isinstance(value, dict) and set(value) == ({'parallel_plan_sha256', 'logical_round', 'attempt',
                                                       'shard', 'partition_sha256'} if partitioned else
                                                      {'parallel_plan_sha256', 'logical_round', 'attempt'})
            and isinstance(value['parallel_plan_sha256'], str)
            and re.fullmatch(r'[0-9a-f]{64}', value['parallel_plan_sha256']) is not None
            and type(value['logical_round']) is int and 1 <= value['logical_round'] <= 3
            and type(value['attempt']) is int and 1 <= value['attempt'] <= 2,
            'invalid parallel round contract')
    if collection is not None:
        require(routing.identity(collection.get('round_contract')) == routing.identity(value),
                'collection round contract differs')
        rounds = collection.get('rounds', [])
        expected = Path(plan['output_root']) / 'collection/round-1/records.json'
        require(len(rounds) == 1 and isinstance(rounds[0], dict)
                and isinstance(rounds[0].get('path'), str)
                and Path(rounds[0]['path']).resolve() == expected.resolve(),
                'parallel collection must contain its one logical round')
    return value


def boundary_pair(path):
    records = list(rows(path))
    require(all(r.get('kind') != 'error' for r in records), 'configuration observer error')
    require(len(records) == 2 and [r.get('stage') for r in records] == ['before', 'after'],
            'missing configuration boundary')
    return [record['value'] for record in records]


def stable_pair(path):
    before, after = boundary_pair(path)
    require(before == after, 'allocation/configuration changed during collection')
    return before


def check_configuration(values, host, pid):
    env = values['job_environment']
    require(env['environment']['JOB_ID'] == host['job_id']
            and env['environment']['NSLOTS'] == str(SLOTS)
            and env['environment']['PE'] == 'omp28'
            and env['library_limits'] == host['library_limits'] == discovery.LIBRARY_LIMITS,
            'job identity, slots, PE or library limits changed')
    lines = [line.split() for line in env['pe_hostfile'].splitlines() if line.strip()]
    require(len(lines) == 1 and len(lines[0]) >= 3 and lines[0][1] == str(SLOTS)
            and hostname(lines[0][0]) == hostname(host['hostname']), 'PE allocation is not one complete host')
    queue = lines[0][2]
    require(queue == env['environment']['QUEUE'] or queue.split('@')[0] == env['environment']['QUEUE'],
            'queue differs from the granted PE hostfile')
    check_host_configuration(values['queue_configuration'], host, queue)
    pe = fields(output(values['parallel_environment'], ['qconf', '-sp', 'omp28']))
    require(pe.get('pe_name') == 'omp28' and pe.get('allocation_rule') == '$pe_slots',
            'parallel environment does not allocate all slots to one host')
    quota = output(values['resource_quota'], ['qconf', '-srqs', 'max_slots_per_host'])
    qfields = fields(quota)
    limits = [line.strip().split() for line in quota.splitlines() if line.strip().startswith('limit ')]
    require(qfields.get('name') == 'max_slots_per_host' and qfields.get('enabled') == 'TRUE'
            and limits == [['limit', 'hosts', '{*}', 'to', 'slots=$num_proc']],
            'quota does not reserve num_proc slots globally across every queue')
    check_affinity(values['affinity'], host, pid)
    return queue


def reservation(raw, host, collection, query_policy=None):
    for key in ('job_environment', 'parallel_environment', 'resource_quota'):
        stable_pair(raw[key])
    pairs = {key: boundary_pair(raw[key]) for key in
             ('job_environment', 'queue_configuration', 'parallel_environment', 'resource_quota', 'affinity')}
    for index in range(2):
        queue = check_configuration({key: values[index] for key, values in pairs.items()},
                                    host, collection['collection_pid'])
    check_scheduler(raw['scheduler'], host, queue, collection, query_policy)
    return {'num_proc': SLOTS, 'slots': SLOTS, 'pe_allocation_rule': '$pe_slots',
            'rqs_enabled': True, 'rqs_all_queues': True, 'rqs_limit': 'num_proc', 'single_host': True}


def check_host_configuration(config, host, queue):
    complexes = {line.split()[0]: line.split() for line in output(config['complexes'], ['qconf', '-sc']).splitlines()
                 if line.strip() and not line.lstrip().startswith('#')}
    require(complexes.get('num_proc', [])[2:6] == ['INT', '==', 'YES', 'NO'],
            'num_proc is not an exact requestable integer host resource')
    require(complexes.get('slots', [])[2:6] == ['INT', '<=', 'YES', 'YES'],
            'slots is not the scheduler consumable resource')
    execution_host = fields(output(config['host'], ['qconf', '-se', host['hostname']]))
    require(hostname(execution_host['hostname']) == hostname(host['hostname']), 'wrong execution host configuration')
    capacities = {}
    for key in ('complex_values', 'load_values'):
        for item in execution_host[key].split(','):
            if '=' in item:
                name, value = (part.strip() for part in item.split('=', 1))
                if name == 'num_proc':
                    require(name not in capacities or capacities[name] == value,
                            'conflicting host num_proc declarations')
                    capacities[name] = value
    require(capacities.get('num_proc') == str(SLOTS) and execution_host.get('processors') == str(SLOTS),
            'execution host num_proc/processors differs from reservation')
    queue_config = fields(output(config['queue'], ['qconf', '-sq', queue.split('@')[0]]))
    require(queue_config['qname'] == queue.split('@')[0], 'wrong queue configuration')


def check_affinity(raw, host, pid):
    status = fields(raw['proc_status'], colon=True)
    require(status['Pid'] == str(pid) and cpu_set(status['Cpus_allowed_list']) == host['affinity']
            and len(host['affinity']) == SLOTS,
            'affinity does not cover the complete reserved host')
    text = output(raw['topology'], ['lscpu', '-p=CPU,CORE,SOCKET,ONLINE'])
    topology = [line.split(',') for line in text.splitlines() if line and not line.startswith('#')]
    require(topology and all(len(row) == 4 and row[3] in {'Y', 'N'} for row in topology), 'invalid CPU topology')
    online = [(int(cpu), int(core), int(socket)) for cpu, core, socket, active in topology if active == 'Y']
    require(sorted(row[0] for row in online) == host['affinity']
            and len({row[1:] for row in online}) == SLOTS, 'topology is not 28 complete physical cores')


def check_scheduler(path, host, queue, collection, query_policy=None):
    snapshots = list(rows(path))
    require(all(snapshot.get('kind') != 'error' for snapshot in snapshots), 'scheduler observer error')
    if query_policy is not None:
        require(query_policy == discovery.SCHEDULER_QUERY_POLICY, 'unknown scheduler query policy')
        snapshots = query_snapshots(snapshots, host, queue)
    else:
        require(all(snapshot.get('kind') != 'scheduler_query' for snapshot in snapshots),
                'scheduler query recovery requires the frozen plan policy')
    require(len(snapshots) >= 2 and snapshots[0].get('stage') == 'before'
            and snapshots[-1].get('stage') == 'after', 'missing scheduler boundary')
    require(snapshots[0]['monotonic_ns'] <= collection['started_monotonic_ns']
            <= collection['completed_monotonic_ns'] <= snapshots[-1]['monotonic_ns'],
            'scheduler census does not cover collection')
    previous = None
    for snapshot in snapshots:
        now = snapshot['monotonic_ns']
        require(previous is None or 0 < now - previous <= 75e9, 'scheduler observation gap')
        previous = now
        check_scheduler_snapshot(snapshot, host, queue)


def check_scheduler_snapshot(snapshot, host, queue):
    check_scheduler_job(snapshot['job'], host)
    check_scheduler_queues(snapshot['queues'], host, queue)


def check_scheduler_job(record, host):
    job = fields(output(record, ['qstat', '-j', host['job_id']]), colon=True)
    require(job['job_number'] == host['job_id'] and job['project'] == 'welfgr'
            and re.fullmatch(r'omp28\s+range:\s*28', job['parallel environment']) is not None,
            'scheduler job or granted request differs')
    resources = dict(item.split('=', 1) for item in job['hard resource_list'].split(','))
    require(resources.get('num_proc') == str(SLOTS), 'job lacks exact num_proc=28 request')
    duration = resources.get('h_rt', '').split(':')
    require(1 <= len(duration) <= 3 and all(s.isdigit() for s in duration), 'missing finite scheduler wall limit')
    seconds = sum(int(value) * 60**index for index, value in enumerate(reversed(duration)))
    require(0 < seconds <= 43200, 'collection scheduler wall limit exceeds twelve hours')


def check_scheduler_queues(record, host, queue):
    xml = ET.fromstring(output(record, ['qstat', '-f', '-xml', '-q',
                        '*@' + host['hostname'], '-u', '*']))
    queues = xml.findall('./queue_info/Queue-List')
    require(queues and all(hostname(q.findtext('name').split('@')[1]) == hostname(host['hostname'])
                           for q in queues), 'scheduler queue census is for another host')
    jobs = [(q.findtext('name'), job) for q in queues for job in q.findall('job_list')]
    require(len(jobs) == 1 and hostname(jobs[0][0].split('@')[1]) == hostname(host['hostname'])
            and jobs[0][0].split('@')[0] == queue.split('@')[0]
            and jobs[0][1].findtext('JB_job_number') == host['job_id']
            and jobs[0][1].findtext('slots') == str(SLOTS), 'scheduler reports co-tenancy or missing full reservation')


def query_arguments(host):
    return [['qstat', '-j', host['job_id']],
            ['qstat', '-f', '-xml', '-q', '*@' + host['hostname'], '-u', '*']]


def query_command(argv, deadline):
    """Retain bytes even for timeout/transport failures; never renew the deadline."""
    started = time.monotonic_ns()
    remaining = min(10 * 10**9, deadline - started)
    require(remaining > 0, 'scheduler query deadline exhausted')
    record = {'argv': argv, 'started_monotonic_ns': started, 'started_wall_time_ns': time.time_ns(),
              'timeout_ns': remaining, 'locale': 'C', 'timezone': 'UTC', 'error': None}
    stdout, stderr, returncode = b'', b'', None
    try:
        result = subprocess.run(argv, capture_output=True, timeout=remaining / 1e9,
                                env={**os.environ, 'LC_ALL': 'C', 'TZ': 'UTC'})
        stdout, stderr, returncode = result.stdout, result.stderr, result.returncode
        status = 'success' if returncode == 0 else 'nonzero'
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b'', error.stderr or b''
        status, record['error'] = 'timeout', str(error)
    except OSError as error:
        status, record['error'] = 'transport', str(error)
    record.update(finished_monotonic_ns=time.monotonic_ns(), finished_wall_time_ns=time.time_ns(),
                  stdout_base64=base64.b64encode(stdout).decode('ascii'),
                  stderr_base64=base64.b64encode(stderr).decode('ascii'),
                  returncode=returncode, status=status)
    return record


def query_output(record, argv, host, queue, index):
    """Every exit-zero component is semantic evidence, including a failed pair."""
    require(record['argv'] == argv and record['locale'] == 'C' and record['timezone'] == 'UTC',
            'scheduler query command identity differs')
    stdout = base64.b64decode(record['stdout_base64'], validate=True)
    stderr = base64.b64decode(record['stderr_base64'], validate=True)
    status, code = record['status'], record['returncode']
    require((status == 'success' and type(code) is int and code == 0 and record['error'] is None)
            or (status == 'nonzero' and type(code) is int and code != 0 and record['error'] is None)
            or (status in {'timeout', 'transport'} and code is None and isinstance(record['error'], str)),
            'scheduler query command status differs')
    if status != 'success':
        return None
    result = {'argv': argv, 'stdout': stdout.decode('utf-8'), 'stderr': stderr.decode('utf-8'),
              'returncode': 0, 'locale': 'C', 'timezone': 'UTC'}
    if index == 0:
        check_scheduler_job(result, host)
    else:
        check_scheduler_queues(result, host, queue)
    return result


def query_deadline(started, previous):
    return min(started + 20 * 10**9, previous + 75 * 10**9) if previous is not None else started + 20 * 10**9


def query_snapshot(envelope, host, queue, previous):
    """Replay the full envelope; a valid first pair cannot be discarded."""
    require(previous is None or (type(previous) is int and previous >= 0), 'invalid prior scheduler timestamp')
    require(envelope.get('kind') == 'scheduler_query' and envelope.get('schema') == 1
            and envelope.get('status') == 'complete' and envelope.get('error') is None
            and envelope.get('stage') in {'before', 'periodic', 'after'}, 'incomplete scheduler query envelope')
    for name in ('started_monotonic_ns', 'finished_monotonic_ns', 'started_wall_time_ns',
                 'finished_wall_time_ns', 'deadline_monotonic_ns'):
        require(type(envelope[name]) is int and envelope[name] >= 0, 'invalid scheduler query timestamp')
    started, finished, deadline = (envelope[name] for name in
                                 ('started_monotonic_ns', 'finished_monotonic_ns', 'deadline_monotonic_ns'))
    require(type(envelope['previous_success_monotonic_ns']) is type(previous)
            and envelope['previous_success_monotonic_ns'] == previous
            and deadline == query_deadline(started, previous) and started < deadline
            and started <= finished <= deadline
            and envelope['started_wall_time_ns'] <= envelope['finished_wall_time_ns'],
            'scheduler query deadline or predecessor differs')
    attempts = envelope['attempts']
    require(isinstance(attempts, list) and 1 <= len(attempts) <= 2, 'scheduler query attempt count differs')
    prior, wall = started, envelope['started_wall_time_ns']
    for attempt_index, attempt in enumerate(attempts):
        require(isinstance(attempt, list) and len(attempt) == 2, 'incomplete scheduler query pair')
        outputs = []
        for index, (record, argv) in enumerate(zip(attempt, query_arguments(host))):
            for name in ('started_monotonic_ns', 'finished_monotonic_ns', 'started_wall_time_ns',
                         'finished_wall_time_ns', 'timeout_ns'):
                require(type(record[name]) is int and record[name] >= 0, 'invalid scheduler command timestamp')
            begin, end = record['started_monotonic_ns'], record['finished_monotonic_ns']
            require(prior <= begin <= end <= finished and 0 < record['timeout_ns'] <= min(10 * 10**9, deadline - begin)
                    and wall <= record['started_wall_time_ns'] <= record['finished_wall_time_ns']
                    <= envelope['finished_wall_time_ns'], 'scheduler command exceeds shared deadline or order')
            outputs.append(query_output(record, argv, host, queue, index))
            prior, wall = end, record['finished_wall_time_ns']
        complete = all(value is not None for value in outputs)
        require(complete == (attempt_index == len(attempts) - 1), 'scheduler discarded success or exhausted retries')
    return {'stage': envelope['stage'], 'monotonic_ns': attempts[-1][0]['started_monotonic_ns'],
            'job': outputs[0], 'queues': outputs[1]}


def query_snapshots(envelopes, host, queue):
    snapshots, previous, finished = [], None, None
    for envelope in envelopes:
        snapshot = query_snapshot(envelope, host, queue, previous)
        require(finished is None or finished <= envelope['started_monotonic_ns'], 'overlapping scheduler queries')
        snapshots.append(snapshot)
        previous, finished = snapshot['monotonic_ns'], envelope['finished_monotonic_ns']
    return snapshots


class SchedulerQueries:
    """One serial scheduler observer; the frozen plan selects this policy."""

    def __init__(self, path, host, queue, failure_event):
        self.path, self.host, self.queue, self.failure_event = path, host, queue, failure_event
        self.previous, self.failed = None, False

    def capture(self, stage):
        require(not self.failed, 'scheduler observer already failed')
        started = time.monotonic_ns()
        envelope = {'kind': 'scheduler_query', 'schema': 1, 'stage': stage,
                    'started_monotonic_ns': started, 'started_wall_time_ns': time.time_ns(),
                    'previous_success_monotonic_ns': self.previous,
                    'deadline_monotonic_ns': query_deadline(started, self.previous),
                    'attempts': [], 'status': 'incomplete', 'error': None}
        try:
            for _ in range(2):
                attempt, outputs = [], []
                envelope['attempts'].append(attempt)
                for index, argv in enumerate(query_arguments(self.host)):
                    record = query_command(argv, envelope['deadline_monotonic_ns'])
                    attempt.append(record)
                    outputs.append(query_output(record, argv, self.host, self.queue, index))
                    require(record['finished_monotonic_ns'] <= envelope['deadline_monotonic_ns'],
                            'scheduler query completed after deadline')
                if all(value is not None for value in outputs):
                    envelope.update(status='complete', finished_monotonic_ns=time.monotonic_ns(),
                                    finished_wall_time_ns=time.time_ns())
                    snapshot = query_snapshot(envelope, self.host, self.queue, self.previous)
                    self.previous = snapshot['monotonic_ns']
                    return snapshot
            raise ValueError('scheduler query exhausted two pairs')
        except Exception as error:
            self.failed = True
            self.failure_event.set()
            envelope.update(status='error', error=str(error), finished_monotonic_ns=time.monotonic_ns(),
                            finished_wall_time_ns=time.time_ns())
            raise
        finally:
            try:
                append(self.path, envelope)
            except Exception:
                self.failed = True
                self.failure_event.set()
                raise


def parsed_processes(row):
    processes = block_host.parse_ps(output(row['ps'], ['ps', '-axo', PS_FIELDS]))
    block_host.attach_ps_arguments(processes, output(row['args'], ['ps', '-axo', PS_ARGS]))
    return {'kind': 'snapshot', 'monotonic_ns': row['monotonic_ns'],
            'finished_monotonic_ns': row['finished_monotonic_ns'], 'processes': processes}


def process_facts(path, plan, collection):
    observations = iter(rows(path))
    first = next(observations)
    require(first['kind'] == 'start' and first['plan_sha256'] == routing.identity(plan)
            and first['policy'] == policy(plan) and first['pid'] == collection['collection_pid'],
            'process observer identity changed')
    prior, start, finish, end = None, None, None, None
    owned, activity = set(), []
    for row in observations:
        if row.get('kind') == 'end':
            require(end is None, 'duplicate observer end')
            end = row
            continue
        require(end is None and row.get('kind') == 'snapshot', 'observer error or invalid sample')
        current = parsed_processes(row)
        now = current['monotonic_ns']
        require(0 <= current['finished_monotonic_ns'] - now <= 7.5e9, 'slow/incomplete process sample')
        if prior is None:
            owner = [p for p in current['processes'] if p['pid'] == first['pid']]
            require(len(owner) == 1, 'observer owner absent')
            owned.add(block_host.identity(owner[0]))
            start = now
        else:
            require(0 < now - prior['monotonic_ns'] <= 7.5e9, 'process observation gap')
        by_pid = {p['pid']: p for p in current['processes']}
        changed = True
        while changed:
            changed = False
            for process in current['processes']:
                parent = by_pid.get(process['ppid'])
                key = block_host.identity(process)
                if parent and block_host.identity(parent) in owned and key not in owned:
                    owned.add(key)
                    changed = True
        hits = block_host.process_activity(([prior] if prior else []) + [current], [], owned, first['policy'])
        # A nonempty prefix suffices to refuse; raw observations retain every hit.
        activity.extend(hits[:max(0, 100 - len(activity))])
        prior, finish = current, now
    require(end is not None and end['collection_sha256'] == routing.identity(collection)
            and end['status'] == 'complete', 'observer does not bind completed collection')
    require(start is not None and start <= collection['started_monotonic_ns']
            <= collection['completed_monotonic_ns'] <= finish, 'collection window is not fully observed')
    return activity


def accounting(raw, collection):
    job = collection['host']
    values = fields(output(raw, ['qacct', '-j', job['job_id']]))
    require(values['jobnumber'] == job['job_id'] and values['project'] == 'welfgr'
            and hostname(values['hostname']) == hostname(job['hostname'])
            and values['slots'] == str(SLOTS) and values['granted_pe'] == 'omp28', 'wrong accounted allocation')
    def instant(value):
        return datetime.strptime(value, '%a %b %d %H:%M:%S %Y').replace(tzinfo=timezone.utc).timestamp() * 1e9
    require(instant(values['start_time']) <= collection['started_wall_time_ns']
            and instant(values['end_time']) + 1e9 >= collection['completed_wall_time_ns'],
            'accounting does not cover collection')
    return {name: int(values[name].split()[0]) for name in ('failed', 'exit_status')}


def derived(plan, collection, raw):
    check_sources(plan)
    contract = round_contract(plan, collection)
    require(collection['plan_sha256'] == routing.identity(plan)
            and collection['status'] == 'complete' and len(collection['rounds']) == (1 if contract else 3),
            'incomplete or different collection')
    require(collection['host']['slots'] == SLOTS and collection['host']['machine'] == 'x86_64',
            'wrong physical allocation or architecture')
    require(set(raw) == KINDS, 'wrong raw evidence coverage')
    paths = {key: discovery.checked(ref) for key, ref in raw.items()}
    return {'reservation': reservation(paths, collection['host'], collection, discovery.scheduler_query_policy(plan)),
            'accounting': accounting(json.loads(paths['accounting'].read_text()), collection),
            'observed_compute_overlap': process_facts(paths['process_observations'], plan, collection),
            'observation_complete': True}


def validate(plan, collection, receipt):
    require(receipt.get('kind') == discovery.KIND + '_isolation'
            and receipt['plan_sha256'] == routing.identity(plan)
            and receipt['collection_sha256'] == routing.identity(collection)
            and receipt['generator'] == plan['receipt_generator']
            and receipt['host'] == collection['host'] and receipt['rounds'] == collection['rounds'],
            'isolation receipt identity changed')
    facts = derived(plan, collection, receipt['raw_evidence'])
    require(all(receipt.get(key) == value for key, value in facts.items()), 'asserted facts differ from raw evidence')
    require(facts['accounting'] == {'failed': 0, 'exit_status': 0}
            and not facts['observed_compute_overlap'], 'failed job or observed competing compute')
    return {'schema': 1, 'status': 'verified', 'plan_sha256': routing.identity(plan),
            'collection_sha256': routing.identity(collection), 'receipt_sha256': routing.identity(receipt)}


def scheduler(host, stage):
    return {'stage': stage, 'monotonic_ns': time.monotonic_ns(),
            'job': command(['qstat', '-j', host['job_id']]),
            'queues': command(['qstat', '-f', '-xml', '-q', '*@' + host['hostname'], '-u', '*'])}


def configuration(directory, host, stage, query_state=None):
    # At exit, bracket the completed measurement before slower configuration
    # reads. At entry, take the live scheduler census last, just before admission.
    census = (query_state.capture(stage) if query_state else scheduler(host, stage)) if stage == 'after' else None
    if census is not None and query_state is None:
        append(directory / 'scheduler.jsonl', census)
    env = {key: os.environ[key] for key in ('JOB_ID', 'NSLOTS', 'PE', 'QUEUE')}
    values = {
        'queue_configuration': {'complexes': command(['qconf', '-sc']),
            'host': command(['qconf', '-se', host['hostname']]),
            'queue': command(['qconf', '-sq', env['QUEUE'].split('@')[0]])},
        'parallel_environment': command(['qconf', '-sp', 'omp28']),
        'resource_quota': command(['qconf', '-srqs', 'max_slots_per_host']),
        'job_environment': {'environment': env, 'library_limits': host['library_limits'],
                            'pe_hostfile': Path(os.environ['PE_HOSTFILE']).read_text()},
        'affinity': {'proc_status': Path('/proc/self/status').read_text(),
                     'topology': command(['lscpu', '-p=CPU,CORE,SOCKET,ONLINE'])},
    }
    for key, value in values.items():
        # Only affinity's stable CPU fields are compared: RSS and CPU clocks in
        # /proc/self/status naturally change while the same process runs.
        append(directory / (key + '.jsonl'), {'stage': stage, 'value': value})
    if census is None:
        census = query_state.capture(stage) if query_state else scheduler(host, stage)
        if query_state is None:
            append(directory / 'scheduler.jsonl', census)
    return values, census


def process_snapshot():
    result = {'kind': 'snapshot', 'monotonic_ns': time.monotonic_ns(), 'wall_time_ns': time.time_ns()}
    result['ps'] = command(['ps', '-axo', PS_FIELDS], timeout=3)
    result['args'] = command(['ps', '-axo', PS_ARGS], timeout=3)
    result['finished_monotonic_ns'] = time.monotonic_ns()
    return result


def observer_error(path, error, failure_event=None):
    if failure_event is not None:
        failure_event.set()
    append(path, {'kind': 'error', 'monotonic_ns': time.monotonic_ns(),
                  'wall_time_ns': time.time_ns(), 'error': str(error)})


def periodic(stop, seconds, action, error_path, initial_ns, failure_event=None):
    deadline = initial_ns / 1e9 + seconds
    while not stop.wait(max(0, deadline - time.monotonic())):
        try:
            action()
        except Exception as error:
            observer_error(error_path, error, failure_event)
        deadline += seconds


def start_observers(stop, directory, host, initial_process_ns, initial_scheduler_ns, failure_event=None,
                    query_state=None):
    process_path, scheduler_path = (directory / name for name in
                                   ('process_observations.jsonl', 'scheduler.jsonl'))
    scheduler_action = (lambda: query_state.capture('periodic')) if query_state else (
                        lambda: append(scheduler_path, scheduler(host, 'periodic')))
    work = [(5, lambda: append(process_path, process_snapshot()), process_path, initial_process_ns),
            (60, scheduler_action, scheduler_path, initial_scheduler_ns)]
    observers = [threading.Thread(target=periodic, args=(stop, interval, action, path, initial_ns),
                 kwargs={'failure_event': failure_event}, daemon=True)
                 for interval, action, path, initial_ns in work]
    for observer in observers:
        observer.start()
    return observers


def observe(plan, proof, operation=None):
    discovery.validate_plan(plan)
    check_sources(plan)
    round_contract(plan)
    host = discovery.host()
    require(host['slots'] == SLOTS and host['library_limits'] == discovery.LIBRARY_LIMITS,
            'collector requires the complete 28-slot job and fixed library limits')
    directory = Path(plan['output_root']) / 'isolation'
    directory.mkdir(parents=True, exist_ok=False)
    query_policy = discovery.scheduler_query_policy(plan)
    failure_event = threading.Event() if operation is not None or query_policy is not None else None
    query_state = (SchedulerQueries(directory / 'scheduler.jsonl', host, os.environ['QUEUE'], failure_event)
                   if query_policy is not None else None)
    query_options = {'query_state': query_state} if query_state is not None else {}
    values, census = configuration(directory, host, 'before', **query_options)
    queue = check_configuration(values, host, os.getpid())
    check_scheduler_snapshot(census, host, queue)
    path = directory / 'process_observations.jsonl'
    append(path, {'kind': 'start', 'pid': os.getpid(), 'plan_sha256': routing.identity(plan), 'policy': policy(plan)})
    initial = process_snapshot()
    append(path, initial)
    parsed = parsed_processes(initial)
    owner = [block_host.identity(p) for p in parsed['processes'] if p['pid'] == os.getpid()]
    require(len(owner) == 1 and not block_host.process_activity([parsed], [], owner, policy(plan)),
            'known competing compute or absent owner at initial admission')
    stop = threading.Event()
    observers = start_observers(stop, directory, host, initial['monotonic_ns'], census['monotonic_ns'],
                                failure_event, **query_options)
    result = None
    try:
        output = Path(plan['output_root']) / 'collection'
        result = (operation(plan, proof, output, failure_event) if operation is not None
                  else discovery.collect(plan, proof, output))
        return result
    finally:
        stop.set()
        if operation is None:
            observers[0].join(timeout=10)
            require(not observers[0].is_alive(), 'process observer did not finish')
            append(path, process_snapshot())
            observers[1].join(timeout=25)
            require(not observers[1].is_alive(), 'scheduler observer did not finish')
            configuration(directory, host, 'after', **query_options)
            append(path, {'kind': 'end', 'status': result['status'] if result else 'incomplete',
                          'collection_sha256': routing.identity(result) if result else None})
        else:
            errors = []
            def retain(action, error_path):
                try:
                    action()
                    return True
                except Exception as error:
                    errors.append(str(error))
                    observer_error(error_path, error, failure_event)
                    return False
            def joined(observer, timeout, name):
                observer.join(timeout=timeout)
                require(not observer.is_alive(), name + ' observer did not finish')
            retain(lambda: joined(observers[0], 10, 'process'), path)
            retain(lambda: append(path, process_snapshot()), path)
            scheduler_joined = retain(lambda: joined(observers[1], 25, 'scheduler'), directory / 'scheduler.jsonl')
            # A failed bounded join must not race a second query against the
            # still-running observer. The legacy cleanup sequence is unchanged.
            if scheduler_joined or query_state is None:
                retain(lambda: configuration(directory, host, 'after', **query_options), directory / 'scheduler.jsonl')
            append(path, {'kind': 'end', 'monotonic_ns': time.monotonic_ns(), 'wall_time_ns': time.time_ns(),
                          'status': result['status'] if result and not failure_event.is_set() else 'incomplete',
                          'collection_sha256': routing.identity(result) if result else None})
            if errors and sys.exc_info()[0] is None:
                raise ValueError('infrastructure evidence failure: ' + '; '.join(errors))


def finalize(plan, collection):
    directory = Path(plan['output_root']) / 'isolation'
    account = directory / 'accounting.json'
    result = command(['qacct', '-j', collection['host']['job_id']], timeout=60)
    with account.open('x') as stream:
        json.dump(result, stream)
    raw = {key: discovery.reference(directory / (key + ('.json' if key == 'accounting' else '.jsonl')))
           for key in KINDS}
    receipt = {'kind': discovery.KIND + '_isolation', 'schema': 1,
        'plan_sha256': routing.identity(plan), 'collection_sha256': routing.identity(collection),
        'generator': plan['receipt_generator'], 'host': collection['host'], 'rounds': collection['rounds'],
        'raw_evidence': raw, **derived(plan, collection, raw)}
    discovery.write(directory / 'receipt.json', receipt)
    validate(plan, collection, receipt)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['collect', 'finalize', 'validate-stdin'])
    parser.add_argument('--plan', type=Path)
    parser.add_argument('--proof', type=Path)
    parser.add_argument('--collection', type=Path)
    args = parser.parse_args()
    if args.command == 'validate-stdin':
        value = json.load(sys.stdin)
        require(set(value) == {'plan', 'collection', 'receipt'}, 'unexpected validation envelope')
        print(json.dumps(validate(**value), allow_nan=False))
        return 0
    plan = discovery.load(args.plan)
    result = observe(plan, discovery.load(args.proof)) if args.command == 'collect' else finalize(plan, discovery.load(args.collection))
    return int(result.get('status') == 'incomplete')


if __name__ == '__main__':
    raise SystemExit(main())
