"""Passive Mac/SCC host observation for fixed-candidate qualification jobs."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import threading
import time
import xml.etree.ElementTree as ET

import block_host

LIBRARY_LIMITS = {name: '1' for name in
                  ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'NUMEXPR_NUM_THREADS')}


class OperationalError(RuntimeError):
    """Missing or invalid host evidence; no scientific or performance verdict."""


class QuietTimeout(OperationalError):
    """Known activity persisted through the bounded wait before a local retry."""


def require(condition, message):
    if not condition:
        raise OperationalError(message)


def command(argv):
    with subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                          start_new_session=True, env={**os.environ, 'LC_ALL': 'C'}) as child:
        try:
            out, err = child.communicate(timeout=10)
        except BaseException:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.communicate()
            raise
        require(child.returncode == 0, f'{argv}: {err.strip()}')
        return out


def fields(text, colon=False):
    result = {}
    for line in text.replace('\\\n', '').splitlines():
        line = line.strip()
        if not line or line.startswith('#') or line in {'{', '}'}:
            continue
        pair = line.split(':', 1) if colon else line.split(None, 1)
        if len(pair) == 2:
            key, value = (part.strip() for part in pair)
            require(key not in result, 'duplicate configuration field: ' + key)
            result[key] = value
    return result


def short_host(name):
    return name.split('.')[0]


def ordinary_activity(processes, previous, owned):
    """Descriptive non-owned activity; never an admission/exclusion rule."""
    prior = {block_host.identity(p): p for p in previous['processes']} if previous else {}
    others = [p for p in processes if block_host.identity(p) not in owned]
    deltas = [max(0., p['cpu_seconds'] - prior[block_host.identity(p)]['cpu_seconds'])
              for p in others if block_host.identity(p) in prior]
    return {'processes': len(others), 'runnable_processes': sum('R' in p['state'] for p in others),
            'cpu_percent_sum': sum(p['cpu_percent'] for p in others),
            'rss_kib_sum': sum(p['rss_kib'] for p in others),
            'cpu_delta_seconds': sum(deltas), 'matched_processes': len(deltas),
            'active_delta_processes': sum(d >= block_host.POLICY['active_cpu_seconds'] for d in deltas)}


def validate_reservation(raw, host, required_threads):
    """Slots plus the all-queue consumable quota establish the reservation."""
    n = host['slots']
    require(n == 28 and 1 <= required_threads <= n, 'expected complete omp28 host and fitting worker count')
    lines = [line.split() for line in raw['pe_hostfile'].splitlines() if line.strip()]
    require(len(lines) == 1 and len(lines[0]) >= 3 and short_host(lines[0][0]) == short_host(host['hostname'])
            and lines[0][1] == str(n), 'PE allocation does not cover one whole host')
    pe = fields(raw['pe'])
    require(pe.get('pe_name') == 'omp28' and pe.get('allocation_rule') == '$pe_slots', 'invalid PE allocation rule')
    quota = fields(raw['quota'])
    limits = [line.strip().split() for line in raw['quota'].splitlines() if line.strip().startswith('limit ')]
    require(quota.get('name') == 'max_slots_per_host' and quota.get('enabled') == 'TRUE'
            and limits == [['limit', 'hosts', '{*}', 'to', 'slots=$num_proc']], 'missing all-queue full-host quota')
    complexes = {line.split()[0]: line.split() for line in raw['complexes'].splitlines()
                 if line.strip() and not line.lstrip().startswith('#')}
    require(complexes.get('num_proc', [])[2:6] == ['INT', '==', 'YES', 'NO']
            and complexes.get('slots', [])[2:6] == ['INT', '<=', 'YES', 'YES'], 'invalid scheduler resource definitions')
    config = fields(raw['host'])
    require(short_host(config.get('hostname', '')) == short_host(host['hostname'])
            and config.get('processors') == str(n), 'wrong host capacity')
    capacities = [part.split('=', 1)[1] for key in ('complex_values', 'load_values')
                  for part in config.get(key, '').split(',') if part.strip().startswith('num_proc=')]
    require(capacities and all(value == str(n) for value in capacities), 'host num_proc capacity differs')
    topology = [line.split(',') for line in raw['topology'].splitlines() if line and not line.startswith('#')]
    require(topology and all(len(row) == 4 and row[3] in {'Y', 'N'} for row in topology), 'invalid CPU topology')
    online = [(int(cpu), int(core), int(sock)) for cpu, core, sock, active in topology if active == 'Y']
    require(sorted(cpu for cpu, _, _ in online) == host['affinity']
            and len({(core, sock) for _, core, sock in online}) == n, 'affinity/topology is not the entire physical host')
    return lines[0][2]


def validate_census(job_text, queue_xml, host):
    job = fields(job_text, colon=True)
    require(job.get('job_number') == host['job_id'] and job.get('project') == 'welfgr', 'wrong scheduler job/project')
    require(job.get('parallel environment', '').split() == ['omp28', 'range:', '28'], 'wrong scheduler PE request')
    resources = dict(part.split('=', 1) for part in job.get('hard resource_list', '').split(',') if '=' in part)
    require(resources.get('num_proc') == '28', 'missing whole-node num_proc request')
    queues = ET.fromstring(queue_xml).findall('./queue_info/Queue-List')
    require(queues and all(short_host(q.findtext('name', '').split('@')[-1]) == short_host(host['hostname'])
                           for q in queues), 'wrong all-queue host census')
    jobs = [(q.findtext('name'), j) for q in queues for j in q.findall('job_list')]
    require(len(jobs) == 1, 'co-tenancy or missing full reservation')
    queue, current = jobs[0]
    require(queue.split('@')[0] == host['queue'].split('@')[0] and current.findtext('JB_job_number') == host['job_id']
            and current.findtext('slots') == '28', 'wrong granted allocation')
    task = current.findtext('tasks') or current.findtext('JAT_task_number')
    require(task == host['task_id'] if host['task_id'] != '0' else task in {None, '', '0'}, 'wrong or missing array task identity')


class HostMonitor:
    """Use around the whole shard, then retain report(); check() fails closed."""
    def __init__(self, output_dir, required_threads, *, execute=command, process_interval=5, census_interval=60):
        self.output = Path(output_dir)
        self.required_threads, self.execute = required_threads, execute
        self.intervals = {'process': process_interval, 'census': census_interval}
        self.sample_locks = {kind: threading.Lock() for kind in self.intervals}
        self.stop = threading.Event()
        self.lock = threading.Lock()
        self.threads, self.errors, self.samples, self.owned = [], [], {}, set()
        self.previous = None
        self.host, self.reservations = {}, []
        self.started = self.finished = None
        self.recording = False
        self.ending = None
        self.local_recovery = False
        self.interference = []
        self.process_policy = copy.deepcopy(block_host.POLICY)
        self.process_policy['executables'] += ['actual_plugin_loader_test', 'actual_plugin_l']

    def _record(self, kind, value):
        with self.lock:
            with (self.output / 'host-observations.jsonl').open('a') as stream:
                stream.write(json.dumps({'kind': kind, **value}, sort_keys=True) + '\n')

    def _reservation(self):
        raw = {'pe_hostfile': Path(os.environ['PE_HOSTFILE']).read_text()}
        for key, argv in [('pe', ['qconf', '-sp', 'omp28']), ('quota', ['qconf', '-srqs', 'max_slots_per_host']),
                          ('complexes', ['qconf', '-sc']), ('host', ['qconf', '-se', self.host['hostname']]),
                          ('topology', ['lscpu', '-p=CPU,CORE,SOCKET,ONLINE'])]:
            raw[key] = self.execute(argv)
        require(sorted(os.sched_getaffinity(0)) == self.host['affinity'], 'CPU affinity changed')
        self._record('reservation', {'monotonic_ns': time.monotonic_ns(), 'raw': raw})
        queue = validate_reservation(raw, self.host, self.required_threads)
        require('queue' not in self.host or self.host['queue'] == queue, 'allocation queue changed')
        self.host['queue'] = queue
        self.reservations.append(raw)

    def _sample(self, kind):
        with self.sample_locks[kind]:
            self._observe(kind)

    def _observe(self, kind):
        start = time.monotonic_ns()
        if kind == 'census':
            raw = {'job': self.execute(['qstat', '-j', self.host['job_id']]),
                   'queues': self.execute(['qstat', '-f', '-xml', '-q', '*@' + self.host['hostname'], '-u', '*'])}
            self._record(kind, {'monotonic_ns': start, 'finished_monotonic_ns': time.monotonic_ns(), 'raw': raw})
            validate_census(raw['job'], raw['queues'], self.host)
        else:
            raw = block_host.snapshot('qualification')
            require('error' not in raw, 'process observation failed: ' + raw.get('error', ''))
            processes = raw['processes']
            if self.previous is None:
                roots = [p for p in processes if p['pid'] == os.getpid()]
                require(len(roots) == 1, 'observer process missing')
                self.owned.add(block_host.identity(roots[0]))
            by_pid = {p['pid']: p for p in processes}
            changed = True
            while changed:
                changed = False
                for process in processes:
                    parent = by_pid.get(process['ppid'])
                    key = block_host.identity(process)
                    if parent and block_host.identity(parent) in self.owned and key not in self.owned:
                        self.owned.add(key)
                        changed = True
            policy = self.process_policy
            hits = block_host.process_activity(([self.previous] if self.previous else []) + [raw], self.owned, policy)
            small_activity = []
            if self.local_recovery:
                # process_activity also visits the previous endpoint. That point
                # was already recorded and must not be attributed to a new cell.
                hits = [h for h in hits if h['end_monotonic_ns'] == raw['monotonic_ns']]
                small_activity = [h for h in block_host.process_activity(
                    ([self.previous] if self.previous else []) + [raw], self.owned, block_host.MAC_POLICY)
                    if h['end_monotonic_ns'] == raw['monotonic_ns']]
                with self.lock:
                    self.interference.extend(copy.deepcopy(hits))
            raw['competing_compute'] = hits
            activity = ordinary_activity(processes, self.previous, self.owned)
            services = []
            for process in processes:
                if block_host.identity(process) in self.owned or process['executable'] != 'codex':
                    continue
                role = block_host.desktop_role(process['command'], process['executable'])
                if role and role['role'] in policy.get('desktop_services', []):
                    services.append(block_host.process_evidence(process) | role)
            self.previous = raw
            self._record('process', {key: raw[key] for key in
                         ('monotonic_ns', 'finished_monotonic_ns', 'competing_compute')} |
                          {'observed_processes': len(processes), 'owned_processes': len(self.owned),
                          'ordinary_activity': activity, 'desktop_services': services} |
                          ({'known_tool_activity': small_activity} if self.local_recovery else {}))
            require(self.local_recovery or not hits, 'known competing compute observed')
        finish = time.monotonic_ns()
        previous = self.samples.get(kind)
        gap = self.intervals[kind] + (15 if kind == 'census' else 2.5)
        require((finish-start)/1e9 <= (20 if kind == 'census' else 7.5), 'slow ' + kind + ' observation')
        boundary = min(start, self.ending) if self.ending is not None else start
        require(previous is None or (boundary-previous['start'])/1e9 <= gap, 'missing ' + kind + ' observation')
        self.samples[kind] = {'start': start, 'finish': finish, 'count': 1 + (previous['count'] if previous else 0)}

    def _error(self, error):
        with self.lock:
            self.errors.append(str(error))
        if self.recording:
            self._record('error', {'error': str(error), 'monotonic_ns': time.monotonic_ns()})

    def _loop(self, kind):
        deadline = time.monotonic() + self.intervals[kind]
        while not self.stop.wait(max(0, deadline-time.monotonic())):
            try:
                self._sample(kind)
            except Exception as error:
                self._error(error)
                return
            deadline += self.intervals[kind]

    def _admit(self):
        require(sys.platform.startswith('linux') and os.environ.get('JOB_ID'), 'SCC monitor requires a Linux SGE job')
        require(all(os.environ.get(k) == v for k, v in LIBRARY_LIMITS.items()), 'library thread limits must be one')
        require(os.environ.get('PE') == 'omp28', 'SCC monitor requires omp28')
        task = os.environ.get('SGE_TASK_ID', '0')
        task = '0' if task == 'undefined' else task
        require(task.isdigit() and os.environ['JOB_ID'].isdigit(), 'invalid SGE identity')
        self.host = {'hostname': socket.gethostname(), 'job_id': os.environ['JOB_ID'], 'task_id': task,
                     'slots': int(os.environ['NSLOTS']), 'affinity': sorted(os.sched_getaffinity(0))}

    def __enter__(self):
        try:
            self._admit()
            self.output.mkdir(parents=True, exist_ok=True)
            with (self.output / 'host-observations.jsonl').open('x'):
                pass
            self.recording = True
            self._reservation()
            for kind in self.intervals:
                self._sample(kind)
            self.started = time.monotonic_ns()
            for kind in self.intervals:
                thread = threading.Thread(target=self._loop, args=(kind,), daemon=True)
                self.threads.append(thread)
                thread.start()
            return self
        except Exception as error:
            self._error(error)
            self._stop()
            raise OperationalError(str(error)) from error

    def _stop(self):
        self.stop.set()
        for thread in self.threads:
            if thread.ident is not None:
                thread.join()  # Each observer subprocess has a bounded timeout and is reaped.

    def __exit__(self, exc_type, exc_value, traceback):
        self.ending = time.monotonic_ns()
        try:
            self._sample('process')  # Observe the native tail before a potentially slow scheduler shutdown.
        except Exception as error:
            self._error(error)
        finally:
            self._stop()
        try:
            if 'census' in self.intervals:
                self._sample('census')
            self._reservation()
            self.finished = time.monotonic_ns()
        except Exception as error:
            self._error(error)
        (self.output / 'host.json').write_text(json.dumps(self.report(), sort_keys=True, indent=2) + '\n')
        if exc_type is None:
            self.check()

    def check(self):
        require(self.started is not None and set(self.samples) == set(self.intervals), 'host observation not started')
        require(not self.errors, '; '.join(self.errors))
        if self.finished is None:
            for kind, sample in self.samples.items():
                require((time.monotonic_ns()-sample['start'])/1e9 <= self.intervals[kind] + 22,
                        'stale ' + kind + ' observer')

    def report(self):
        result = {'schema': 1, 'host': copy.deepcopy(self.host), 'started_monotonic_ns': self.started,
                'finished_monotonic_ns': self.finished, 'measurement_end_monotonic_ns': self.ending, 'samples': copy.deepcopy(self.samples),
                'errors': list(self.errors), 'isolation_ok': bool(self.started and self.finished and not self.errors),
                'process_policy': copy.deepcopy(self.process_policy),
                'observations': 'host-observations.jsonl', 'accounting_required': 'job_id' in self.host,
                'scope': 'Sampled known competing compute; ordinary OS activity retained. SCC also requires a whole-node reservation.'}
        if self.local_recovery:
            with self.lock:
                result['interference'] = copy.deepcopy(self.interference)
            result['cell_eligibility_required'] = True
            result['scope'] = ('Sampled Mac activity with per-cell eligibility; isolation_ok only establishes '
                               'observer completion. Ordinary and lesser known-tool activity is retained noise.')
        return result


class MacMonitor(HostMonitor):
    """Focused Mac observation shares the SCC process/lifecycle implementation."""
    def __init__(self, output_dir, required_threads, *, recovery=False, **kwargs):
        super().__init__(output_dir, required_threads, **kwargs)
        self.intervals.pop('census')
        self.local_recovery = recovery
        self.process_policy = copy.deepcopy(block_host.MAC_RECOVERY_POLICY if recovery else block_host.MAC_POLICY)
        self.process_policy['executables'] += ['actual_plugin_loader_test', 'actual_plugin_l']

    def _admit(self):
        require(sys.platform == 'darwin', 'Mac monitor requires Darwin')
        require(type(self.required_threads) is int and self.required_threads > 0, 'invalid worker count')
        self.host = {'hostname': socket.gethostname(), 'platform': 'Darwin', 'cpus': os.cpu_count()}

    def _reservation(self):
        require(os.cpu_count() == self.host['cpus'], 'Mac CPU availability changed')

    def interference_between(self, start_ns, end_ns):
        """Seal a cell with a tail sample; eligibility never depends on its timings."""
        require(self.local_recovery, 'cell eligibility requires the recovery observer')
        require(type(start_ns) is type(end_ns) is int and 0 < start_ns <= end_ns,
                'invalid cell observation interval')
        self.check()
        try:
            self._sample('process')
            require(self.previous['monotonic_ns'] >= end_ns, 'cell tail was not observed')
        except Exception as error:
            self._error(error)
            raise
        self.check()
        with self.lock:
            return copy.deepcopy([h for h in self.interference
                                  if h['start_monotonic_ns'] <= end_ns and h['end_monotonic_ns'] >= start_ns])

    def await_quiet(self, deadline_seconds=120):
        """Wait for two clear sampled intervals, bounded by the runner deadline too."""
        require(self.local_recovery and 0 < deadline_seconds <= 120, 'invalid retry wait')
        end = time.monotonic() + deadline_seconds
        started = boundary = time.monotonic_ns()
        clean = []
        while time.monotonic() < end:
            self.check()
            if self.stop.wait(min(self.intervals['process'], max(0, end-time.monotonic()))):
                raise OperationalError('observer stopped during retry wait')
            finish = time.monotonic_ns()
            self.interference_between(boundary, finish)
            with self.lock:
                # Include runnable activity at the new tail endpoint, even
                # though that endpoint is later than the requested seal time.
                hits = [h for h in self.interference if h['end_monotonic_ns'] >= boundary]
            tail = self.previous['monotonic_ns']
            clean = [] if hits else [*clean, {'start_monotonic_ns': boundary, 'end_monotonic_ns': tail}]
            if time.monotonic() > end:
                break
            if len(clean) == 2:
                return {'started_monotonic_ns': started, 'finished_monotonic_ns': time.monotonic_ns(),
                        'clean_intervals': clean}
            boundary = tail
        raise QuietTimeout('known activity did not clear within the retry wait')
